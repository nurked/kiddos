//! A small `wasi_snapshot_preview1`, mapped onto the machine.
//!
//! Programs built against a real libc (wasi-libc) call these instead of
//! the `kiddos` module for printing, files, the clock and exit. Nothing
//! here reaches the host: stdout is the process's stdout, files are the
//! virtual drive under the process's permissions, the clock is the
//! machine's. There is no networking, no host directories, no threads.
//! Enough for `printf`, `fopen`/`fread`/`fwrite`, `malloc`, `time` and
//! `exit`; unsupported calls return `ENOSYS`.
//!
//! One directory is preopened: `/`, so relative paths are absolute paths
//! on the drive (wasi-libc's idea of the current directory starts at `/`).

use crate::host::{Exit, State};
use kiddos_kernel::Console;
use std::collections::HashMap;
use wasmtime::{Caller, Linker};

pub const MODULE: &str = "wasi_snapshot_preview1";

// errno
const SUCCESS: i32 = 0;
const EACCES: i32 = 2;
const EBADF: i32 = 8;
const EEXIST: i32 = 20;
const EINVAL: i32 = 28;
const EIO: i32 = 29;
const EISDIR: i32 = 31;
const ENOENT: i32 = 44;
const ENOSYS: i32 = 52;
const ENOTDIR: i32 = 54;
const ENOTEMPTY: i32 = 55;
const ESPIPE: i32 = 70;

// filetype
const FT_CHARACTER_DEVICE: u8 = 2;
const FT_DIRECTORY: u8 = 3;
const FT_REGULAR_FILE: u8 = 4;

// oflags / fdflags / whence
const O_CREAT: i32 = 1;
const O_DIRECTORY: i32 = 2;
const O_EXCL: i32 = 4;
const O_TRUNC: i32 = 8;
const FD_APPEND: i32 = 1;

const PREOPEN_FD: i32 = 3;
const PREOPEN_NAME: &str = "/";

/// A file opened through WASI: whole contents in memory, written back to
/// the drive on close or sync.
pub struct OpenFile {
    path: String,
    data: Vec<u8>,
    pos: usize,
    dir: bool,
    writable: bool,
    append: bool,
    dirty: bool,
}

#[derive(Default)]
pub struct Files {
    open: HashMap<i32, OpenFile>,
    next_fd: i32,
}

impl Files {
    fn alloc(&mut self, f: OpenFile) -> i32 {
        if self.next_fd <= PREOPEN_FD {
            self.next_fd = PREOPEN_FD + 1;
        }
        let fd = self.next_fd;
        self.next_fd += 1;
        self.open.insert(fd, f);
        fd
    }

    /// Write back anything still dirty (a program that exits without
    /// closing its files).
    pub fn flush_all(&mut self, proc: &kiddos_kernel::Proc) {
        for f in self.open.values_mut() {
            flush(proc, f);
        }
    }
}

fn flush(proc: &kiddos_kernel::Proc, f: &mut OpenFile) {
    if f.dirty && f.writable && !f.dir {
        let _ = proc.fs().write(&f.path, &f.data);
        f.dirty = false;
    }
}

fn memory(caller: &mut Caller<'_, State>) -> anyhow::Result<wasmtime::Memory> {
    caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("The program has no memory export."))
}

fn read_mem(caller: &mut Caller<'_, State>, ptr: i32, len: i32) -> anyhow::Result<Vec<u8>> {
    let mem = memory(caller)?;
    let (start, len) = (ptr.max(0) as usize, len.max(0) as usize);
    let data = mem.data(&*caller);
    let end = start
        .checked_add(len)
        .filter(|e| *e <= data.len())
        .ok_or(wasmtime::Trap::MemoryOutOfBounds)?;
    Ok(data[start..end].to_vec())
}

fn write_mem(caller: &mut Caller<'_, State>, ptr: i32, bytes: &[u8]) -> anyhow::Result<()> {
    let mem = memory(caller)?;
    mem.write(&mut *caller, ptr.max(0) as usize, bytes)
        .map_err(|_| wasmtime::Trap::MemoryOutOfBounds.into())
}

fn write_u32(caller: &mut Caller<'_, State>, ptr: i32, v: u32) -> anyhow::Result<()> {
    write_mem(caller, ptr, &v.to_le_bytes())
}

fn write_u64(caller: &mut Caller<'_, State>, ptr: i32, v: u64) -> anyhow::Result<()> {
    write_mem(caller, ptr, &v.to_le_bytes())
}

fn read_path(caller: &mut Caller<'_, State>, ptr: i32, len: i32) -> anyhow::Result<String> {
    let bytes = read_mem(caller, ptr, len)?;
    let s = String::from_utf8_lossy(&bytes).to_string();
    Ok(resolve(&s))
}

/// Paths come relative to the preopened `/`; make them absolute and tidy.
fn resolve(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            x => out.push(x),
        }
    }
    format!("/{}", out.join("/"))
}

/// Gather the iovec list into one buffer (for writes) or a list of
/// (ptr, len) (for reads).
fn iovecs(caller: &mut Caller<'_, State>, ptr: i32, count: i32) -> anyhow::Result<Vec<(i32, i32)>> {
    let count = count.clamp(0, 1024);
    let raw = read_mem(caller, ptr, count * 8)?;
    Ok(raw
        .chunks_exact(8)
        .map(|c| {
            (
                i32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                i32::from_le_bytes([c[4], c[5], c[6], c[7]]),
            )
        })
        .collect())
}

fn vfs_errno(e: &kiddos_kernel::VfsError) -> i32 {
    use kiddos_kernel::VfsError as V;
    match e {
        V::NotFound(_) => ENOENT,
        V::IsADir(_) => EISDIR,
        V::NotADir(_) => ENOTDIR,
        V::Permission(_) | V::NotPermitted(_) => EACCES,
        V::Exists(_) => EEXIST,
        V::NotEmpty(_) => ENOTEMPTY,
        _ => EIO,
    }
}

/// `argv` / `environ` as a nul-separated block plus offsets.
fn string_block(items: &[String]) -> (Vec<u8>, Vec<u32>) {
    let mut buf = Vec::new();
    let mut offs = Vec::new();
    for s in items {
        offs.push(buf.len() as u32);
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
    }
    (buf, offs)
}

fn write_string_block(
    caller: &mut Caller<'_, State>,
    items: &[String],
    ptrs: i32,
    buf_ptr: i32,
) -> anyhow::Result<i32> {
    let (buf, offs) = string_block(items);
    write_mem(caller, buf_ptr, &buf)?;
    for (i, off) in offs.iter().enumerate() {
        write_u32(caller, ptrs + i as i32 * 4, buf_ptr as u32 + off)?;
    }
    Ok(SUCCESS)
}

pub fn link(l: &mut Linker<State>) -> anyhow::Result<()> {
    let m = MODULE;

    // ---- process ---------------------------------------------------------
    l.func_wrap(
        m,
        "proc_exit",
        |_c: Caller<'_, State>, code: i32| -> anyhow::Result<()> { Err(Exit(code).into()) },
    )?;
    l.func_wrap(
        m,
        "args_sizes_get",
        |mut c: Caller<'_, State>, argc: i32, size: i32| -> anyhow::Result<i32> {
            let argv = c.data().proc.argv.clone();
            let (buf, _) = string_block(&argv);
            write_u32(&mut c, argc, argv.len() as u32)?;
            write_u32(&mut c, size, buf.len() as u32)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "args_get",
        |mut c: Caller<'_, State>, ptrs: i32, buf: i32| -> anyhow::Result<i32> {
            let argv = c.data().proc.argv.clone();
            write_string_block(&mut c, &argv, ptrs, buf)
        },
    )?;
    l.func_wrap(
        m,
        "environ_sizes_get",
        |mut c: Caller<'_, State>, count: i32, size: i32| -> anyhow::Result<i32> {
            let env = environ(&c);
            let (buf, _) = string_block(&env);
            write_u32(&mut c, count, env.len() as u32)?;
            write_u32(&mut c, size, buf.len() as u32)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "environ_get",
        |mut c: Caller<'_, State>, ptrs: i32, buf: i32| -> anyhow::Result<i32> {
            let env = environ(&c);
            write_string_block(&mut c, &env, ptrs, buf)
        },
    )?;

    // ---- clock -----------------------------------------------------------
    l.func_wrap(
        m,
        "clock_time_get",
        |mut c: Caller<'_, State>, _id: i32, _precision: i64, out: i32| -> anyhow::Result<i32> {
            let ns = c.data().proc.tick().saturating_mul(1_000_000);
            write_u64(&mut c, out, ns)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "clock_res_get",
        |mut c: Caller<'_, State>, _id: i32, out: i32| -> anyhow::Result<i32> {
            write_u64(&mut c, out, 1_000_000)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "random_get",
        |mut c: Caller<'_, State>, buf: i32, len: i32| -> anyhow::Result<i32> {
            let len = len.clamp(0, 1 << 20) as usize;
            let mut bytes = Vec::with_capacity(len);
            let mut x = c.data().proc.tick() ^ 0xA5A5_5A5A_1234_5678;
            for _ in 0..len {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                bytes.push((x >> 24) as u8);
            }
            write_mem(&mut c, buf, &bytes)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(m, "sched_yield", |c: Caller<'_, State>| -> anyhow::Result<i32> {
        c.data().proc.sleep(1).map_err(|_| interrupted())?;
        Ok(SUCCESS)
    })?;
    // poll_oneoff: only clock subscriptions (that is `sleep`/`nanosleep`)
    l.func_wrap(
        m,
        "poll_oneoff",
        |mut c: Caller<'_, State>, subs: i32, events: i32, n: i32, nevents: i32| -> anyhow::Result<i32> {
            let n = n.clamp(0, 64);
            let mut written = 0;
            for i in 0..n {
                let sub = read_mem(&mut c, subs + i * 48, 48)?;
                let userdata = &sub[0..8];
                let tag = sub[8];
                if tag == 0 {
                    // clock: timeout at offset 24 (ns), flags at 40 (bit 0 = absolute)
                    let timeout = u64::from_le_bytes(sub[24..32].try_into().unwrap());
                    let abs = sub[40] & 1 != 0;
                    let now_ns = c.data().proc.tick().saturating_mul(1_000_000);
                    let wait_ns = if abs { timeout.saturating_sub(now_ns) } else { timeout };
                    let ms = wait_ns.div_ceil(1_000_000);
                    c.data().proc.sleep(ms).map_err(|_| interrupted())?;
                }
                // event: userdata, error u16 = 0, type u8 = tag
                let mut ev = vec![0u8; 32];
                ev[0..8].copy_from_slice(userdata);
                ev[10] = tag;
                write_mem(&mut c, events + written * 32, &ev)?;
                written += 1;
            }
            write_u32(&mut c, nevents, written as u32)?;
            Ok(SUCCESS)
        },
    )?;

    // ---- descriptors -----------------------------------------------------
    l.func_wrap(
        m,
        "fd_prestat_get",
        |mut c: Caller<'_, State>, fd: i32, out: i32| -> anyhow::Result<i32> {
            if fd != PREOPEN_FD {
                return Ok(EBADF);
            }
            let mut buf = [0u8; 8];
            buf[4..8].copy_from_slice(&(PREOPEN_NAME.len() as u32).to_le_bytes());
            write_mem(&mut c, out, &buf)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_prestat_dir_name",
        |mut c: Caller<'_, State>, fd: i32, path: i32, len: i32| -> anyhow::Result<i32> {
            if fd != PREOPEN_FD {
                return Ok(EBADF);
            }
            let n = (len.max(0) as usize).min(PREOPEN_NAME.len());
            write_mem(&mut c, path, &PREOPEN_NAME.as_bytes()[..n])?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_fdstat_get",
        |mut c: Caller<'_, State>, fd: i32, out: i32| -> anyhow::Result<i32> {
            let ft = match fd {
                0..=2 => FT_CHARACTER_DEVICE,
                PREOPEN_FD => FT_DIRECTORY,
                _ => match c.data().files.open.get(&fd) {
                    Some(f) if f.dir => FT_DIRECTORY,
                    Some(_) => FT_REGULAR_FILE,
                    None => return Ok(EBADF),
                },
            };
            let mut buf = [0u8; 24];
            buf[0] = ft;
            buf[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
            buf[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
            write_mem(&mut c, out, &buf)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_fdstat_set_flags",
        |_c: Caller<'_, State>, _fd: i32, _flags: i32| -> i32 { SUCCESS },
    )?;
    l.func_wrap(
        m,
        "fd_filestat_get",
        |mut c: Caller<'_, State>, fd: i32, out: i32| -> anyhow::Result<i32> {
            let (ft, size) = match fd {
                0..=2 => (FT_CHARACTER_DEVICE, 0),
                PREOPEN_FD => (FT_DIRECTORY, 0),
                _ => match c.data().files.open.get(&fd) {
                    Some(f) if f.dir => (FT_DIRECTORY, 0),
                    Some(f) => (FT_REGULAR_FILE, f.data.len() as u64),
                    None => return Ok(EBADF),
                },
            };
            write_mem(&mut c, out, &filestat(ft, size))?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "path_filestat_get",
        |mut c: Caller<'_, State>, _fd: i32, _flags: i32, path: i32, len: i32, out: i32| -> anyhow::Result<i32> {
            let p = read_path(&mut c, path, len)?;
            let st = match c.data().proc.fs().stat(&p) {
                Ok(st) => st,
                Err(e) => return Ok(vfs_errno(&e)),
            };
            let ft = if st.kind == kiddos_vfs::Kind::Dir {
                FT_DIRECTORY
            } else {
                FT_REGULAR_FILE
            };
            write_mem(&mut c, out, &filestat(ft, st.size))?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "path_open",
        |mut c: Caller<'_, State>,
         _dirfd: i32,
         _dirflags: i32,
         path: i32,
         len: i32,
         oflags: i32,
         _rights: i64,
         _rights_inh: i64,
         fdflags: i32,
         out: i32|
         -> anyhow::Result<i32> {
            let p = read_path(&mut c, path, len)?;
            let fs = c.data().proc.fs();
            let existing = fs.stat(&p).ok();
            let is_dir = existing
                .as_ref()
                .map(|s| s.kind == kiddos_vfs::Kind::Dir)
                .unwrap_or(false);
            if oflags & O_DIRECTORY != 0 || is_dir {
                if existing.is_none() {
                    return Ok(ENOENT);
                }
                if !is_dir {
                    return Ok(ENOTDIR);
                }
                let fd = c.data_mut().files.alloc(OpenFile {
                    path: p,
                    data: Vec::new(),
                    pos: 0,
                    dir: true,
                    writable: false,
                    append: false,
                    dirty: false,
                });
                write_u32(&mut c, out, fd as u32)?;
                return Ok(SUCCESS);
            }
            if existing.is_some() && oflags & O_CREAT != 0 && oflags & O_EXCL != 0 {
                return Ok(EEXIST);
            }
            let mut data = match (&existing, oflags & O_CREAT != 0) {
                (Some(_), _) => match fs.read(&p) {
                    Ok(d) => d,
                    Err(e) => return Ok(vfs_errno(&e)),
                },
                (None, true) => {
                    if let Err(e) = fs.write(&p, b"") {
                        return Ok(vfs_errno(&e));
                    }
                    Vec::new()
                }
                (None, false) => return Ok(ENOENT),
            };
            if oflags & O_TRUNC != 0 {
                data.clear();
            }
            // writable if the drive lets us (checked by appending nothing);
            // a read-only open of a file we may not write is still fine
            let writable = fs.append(&p, b"").is_ok();
            let append = fdflags & FD_APPEND != 0;
            let dirty = oflags & O_TRUNC != 0;
            let fd = c.data_mut().files.alloc(OpenFile {
                path: p,
                data,
                pos: 0,
                dir: false,
                writable,
                append,
                dirty,
            });
            write_u32(&mut c, out, fd as u32)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(m, "fd_close", |mut c: Caller<'_, State>, fd: i32| -> i32 {
        if fd <= PREOPEN_FD {
            return SUCCESS;
        }
        let proc = c.data().proc.clone();
        match c.data_mut().files.open.remove(&fd) {
            Some(mut f) => {
                flush(&proc, &mut f);
                SUCCESS
            }
            None => EBADF,
        }
    })?;
    l.func_wrap(m, "fd_sync", |mut c: Caller<'_, State>, fd: i32| -> i32 {
        let proc = c.data().proc.clone();
        match c.data_mut().files.open.get_mut(&fd) {
            Some(f) => {
                flush(&proc, f);
                SUCCESS
            }
            None => EBADF,
        }
    })?;
    l.func_wrap(m, "fd_datasync", |mut c: Caller<'_, State>, fd: i32| -> i32 {
        let proc = c.data().proc.clone();
        match c.data_mut().files.open.get_mut(&fd) {
            Some(f) => {
                flush(&proc, f);
                SUCCESS
            }
            None => EBADF,
        }
    })?;
    l.func_wrap(
        m,
        "fd_write",
        |mut c: Caller<'_, State>, fd: i32, iovs: i32, n: i32, nwritten: i32| -> anyhow::Result<i32> {
            let list = iovecs(&mut c, iovs, n)?;
            let mut bytes = Vec::new();
            for (p, l) in list {
                bytes.extend(read_mem(&mut c, p, l)?);
            }
            match fd {
                1 => c.data().proc.print(&String::from_utf8_lossy(&bytes)),
                2 => c.data().proc.eprint(&String::from_utf8_lossy(&bytes)),
                0 | PREOPEN_FD => return Ok(EBADF),
                _ => {
                    let Some(f) = c.data_mut().files.open.get_mut(&fd) else {
                        return Ok(EBADF);
                    };
                    if f.dir {
                        return Ok(EISDIR);
                    }
                    if !f.writable {
                        return Ok(EACCES);
                    }
                    if f.append {
                        f.pos = f.data.len();
                    }
                    let end = f.pos + bytes.len();
                    if end > f.data.len() {
                        f.data.resize(end, 0);
                    }
                    f.data[f.pos..end].copy_from_slice(&bytes);
                    f.pos = end;
                    f.dirty = true;
                }
            }
            write_u32(&mut c, nwritten, bytes.len() as u32)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_read",
        |mut c: Caller<'_, State>, fd: i32, iovs: i32, n: i32, nread: i32| -> anyhow::Result<i32> {
            let list = iovecs(&mut c, iovs, n)?;
            let mut total = 0usize;
            match fd {
                0 => {
                    // one line of typing per read, like a terminal
                    let line = c.data().proc.readline("").map_err(|_| interrupted())?;
                    let Some(line) = line else {
                        write_u32(&mut c, nread, 0)?;
                        return Ok(SUCCESS);
                    };
                    let mut bytes = line.into_bytes();
                    bytes.push(b'\n');
                    let mut off = 0;
                    for (p, l) in list {
                        if off >= bytes.len() {
                            break;
                        }
                        let take = (l.max(0) as usize).min(bytes.len() - off);
                        write_mem(&mut c, p, &bytes[off..off + take])?;
                        off += take;
                    }
                    total = off;
                }
                1 | 2 | PREOPEN_FD => return Ok(EBADF),
                _ => {
                    for (p, l) in list {
                        let chunk = {
                            let Some(f) = c.data_mut().files.open.get_mut(&fd) else {
                                return Ok(EBADF);
                            };
                            if f.dir {
                                return Ok(EISDIR);
                            }
                            let take = (l.max(0) as usize).min(f.data.len().saturating_sub(f.pos));
                            let chunk = f.data[f.pos..f.pos + take].to_vec();
                            f.pos += take;
                            chunk
                        };
                        if chunk.is_empty() {
                            break;
                        }
                        write_mem(&mut c, p, &chunk)?;
                        total += chunk.len();
                    }
                }
            }
            write_u32(&mut c, nread, total as u32)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_seek",
        |mut c: Caller<'_, State>, fd: i32, offset: i64, whence: i32, out: i32| -> anyhow::Result<i32> {
            if fd <= PREOPEN_FD {
                return Ok(ESPIPE);
            }
            let new_pos = {
                let Some(f) = c.data_mut().files.open.get_mut(&fd) else {
                    return Ok(EBADF);
                };
                let base = match whence {
                    0 => 0i64,
                    1 => f.pos as i64,
                    2 => f.data.len() as i64,
                    _ => return Ok(EINVAL),
                };
                let np = base + offset;
                if np < 0 {
                    return Ok(EINVAL);
                }
                f.pos = np as usize;
                np as u64
            };
            write_u64(&mut c, out, new_pos)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_tell",
        |mut c: Caller<'_, State>, fd: i32, out: i32| -> anyhow::Result<i32> {
            let pos = match c.data().files.open.get(&fd) {
                Some(f) => f.pos as u64,
                None => return Ok(EBADF),
            };
            write_u64(&mut c, out, pos)?;
            Ok(SUCCESS)
        },
    )?;
    l.func_wrap(
        m,
        "fd_readdir",
        |_c: Caller<'_, State>, _fd: i32, _buf: i32, _len: i32, _cookie: i64, _out: i32| -> i32 { ENOSYS },
    )?;

    // ---- paths -----------------------------------------------------------
    l.func_wrap(
        m,
        "path_create_directory",
        |mut c: Caller<'_, State>, _fd: i32, path: i32, len: i32| -> anyhow::Result<i32> {
            let p = read_path(&mut c, path, len)?;
            Ok(match c.data().proc.fs().mkdir(&p) {
                Ok(()) => SUCCESS,
                Err(e) => vfs_errno(&e),
            })
        },
    )?;
    l.func_wrap(
        m,
        "path_unlink_file",
        |mut c: Caller<'_, State>, _fd: i32, path: i32, len: i32| -> anyhow::Result<i32> {
            let p = read_path(&mut c, path, len)?;
            Ok(match c.data().proc.fs().unlink(&p) {
                Ok(()) => SUCCESS,
                Err(e) => vfs_errno(&e),
            })
        },
    )?;
    l.func_wrap(
        m,
        "path_remove_directory",
        |mut c: Caller<'_, State>, _fd: i32, path: i32, len: i32| -> anyhow::Result<i32> {
            let p = read_path(&mut c, path, len)?;
            Ok(match c.data().proc.fs().rmdir(&p) {
                Ok(()) => SUCCESS,
                Err(e) => vfs_errno(&e),
            })
        },
    )?;
    l.func_wrap(
        m,
        "path_rename",
        |mut c: Caller<'_, State>,
         _fd: i32,
         path: i32,
         len: i32,
         _fd2: i32,
         path2: i32,
         len2: i32|
         -> anyhow::Result<i32> {
            let from = read_path(&mut c, path, len)?;
            let to = read_path(&mut c, path2, len2)?;
            Ok(match c.data().proc.fs().rename(&from, &to) {
                Ok(()) => SUCCESS,
                Err(e) => vfs_errno(&e),
            })
        },
    )?;
    l.func_wrap(
        m,
        "path_readlink",
        |_c: Caller<'_, State>, _fd: i32, _p: i32, _l: i32, _b: i32, _bl: i32, _o: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "path_symlink",
        |_c: Caller<'_, State>, _p: i32, _l: i32, _fd: i32, _p2: i32, _l2: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "path_link",
        |_c: Caller<'_, State>, _fd: i32, _f: i32, _p: i32, _l: i32, _fd2: i32, _p2: i32, _l2: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "fd_pread",
        |_c: Caller<'_, State>, _fd: i32, _i: i32, _n: i32, _o: i64, _r: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "fd_pwrite",
        |_c: Caller<'_, State>, _fd: i32, _i: i32, _n: i32, _o: i64, _r: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "fd_advise",
        |_c: Caller<'_, State>, _fd: i32, _o: i64, _l: i64, _a: i32| -> i32 { SUCCESS },
    )?;
    l.func_wrap(
        m,
        "fd_allocate",
        |_c: Caller<'_, State>, _fd: i32, _o: i64, _l: i64| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "fd_filestat_set_size",
        |mut c: Caller<'_, State>, fd: i32, size: i64| -> i32 {
            match c.data_mut().files.open.get_mut(&fd) {
                Some(f) => {
                    f.data.resize(size.max(0) as usize, 0);
                    f.dirty = true;
                    SUCCESS
                }
                None => EBADF,
            }
        },
    )?;
    l.func_wrap(
        m,
        "fd_filestat_set_times",
        |_c: Caller<'_, State>, _fd: i32, _a: i64, _m: i64, _f: i32| -> i32 { SUCCESS },
    )?;
    l.func_wrap(
        m,
        "path_filestat_set_times",
        |_c: Caller<'_, State>, _fd: i32, _fl: i32, _p: i32, _l: i32, _a: i64, _m: i64, _f: i32| -> i32 { SUCCESS },
    )?;
    l.func_wrap(m, "fd_renumber", |_c: Caller<'_, State>, _a: i32, _b: i32| -> i32 {
        ENOSYS
    })?;
    l.func_wrap(m, "proc_raise", |_c: Caller<'_, State>, _sig: i32| -> i32 { ENOSYS })?;
    l.func_wrap(
        m,
        "sock_accept",
        |_c: Caller<'_, State>, _fd: i32, _fl: i32, _o: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "sock_recv",
        |_c: Caller<'_, State>, _fd: i32, _a: i32, _b: i32, _c2: i32, _d: i32, _e: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(
        m,
        "sock_send",
        |_c: Caller<'_, State>, _fd: i32, _a: i32, _b: i32, _c2: i32, _d: i32| -> i32 { ENOSYS },
    )?;
    l.func_wrap(m, "sock_shutdown", |_c: Caller<'_, State>, _fd: i32, _h: i32| -> i32 {
        ENOSYS
    })?;
    Ok(())
}

fn environ(c: &Caller<'_, State>) -> Vec<String> {
    c.data()
        .proc
        .env_all()
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect()
}

fn filestat(ft: u8, size: u64) -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[16] = ft;
    buf[24..32].copy_from_slice(&1u64.to_le_bytes());
    buf[32..40].copy_from_slice(&size.to_le_bytes());
    buf
}

fn interrupted() -> anyhow::Error {
    wasmtime::Trap::Interrupt.into()
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn resolves_paths_from_the_root() {
        assert_eq!(resolve("games/doom/x.wad"), "/games/doom/x.wad");
        assert_eq!(resolve("/a/./b/../c"), "/a/c");
        assert_eq!(resolve(""), "/");
        assert_eq!(resolve("../../etc"), "/etc");
    }
}
