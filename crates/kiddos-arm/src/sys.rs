//! System calls: what `svc #0` reaches. Linux's numbers where Linux has
//! one (64 write, 93 exit), so what a kid learns here is true on a real
//! ARM box; KidDOS-only calls from 1000 up for the screen and the keys.
//! `man syscalls` is the same table in kid words.

use crate::vm::{Fault, Sys, Vm};
use kiddos_kernel::{Console, Proc};
use rand::RngCore;

pub const SYS_READ: u64 = 63;
pub const SYS_WRITE: u64 = 64;
pub const SYS_EXIT: u64 = 93;
pub const SYS_NANOSLEEP: u64 = 101;
pub const SYS_GETRANDOM: u64 = 278;
pub const SYS_READKEY: u64 = 1000;
pub const SYS_GETKEY: u64 = 1001;
pub const SYS_SLEEP: u64 = 1002;
pub const SYS_BEEP: u64 = 1003;
pub const SYS_TICK: u64 = 1004;
pub const SYS_RANDOM: u64 = 1005;
pub const SYS_READFILE: u64 = 1006;
pub const SYS_WRITEFILE: u64 = 1007;
pub const SYS_PUT: u64 = 1008;
pub const SYS_CURSOR: u64 = 1009;
pub const SYS_CLEAR: u64 = 1010;
pub const SYS_COLOR: u64 = 1011;
pub const SYS_SIZE: u64 = 1012;
pub const SYS_SPEAK: u64 = 1013;

/// `(number, name, arguments, result)` for `man syscalls` and the debugger.
pub const TABLE: &[(u64, &str, &str, &str)] = &[
    (
        SYS_READ,
        "read",
        "x0 fd (0 = keyboard), x1 buffer, x2 size",
        "bytes read; 0 at the end",
    ),
    (
        SYS_WRITE,
        "write",
        "x0 fd (1 = screen, 2 = errors), x1 text, x2 length",
        "bytes written",
    ),
    (SYS_EXIT, "exit", "x0 exit code", "never returns"),
    (
        SYS_NANOSLEEP,
        "nanosleep",
        "x0 -> [seconds, nanoseconds] (two .quad)",
        "0",
    ),
    (SYS_GETRANDOM, "getrandom", "x0 buffer, x1 size", "bytes filled"),
    (SYS_READKEY, "readkey", "-", "the key (waits for one)"),
    (SYS_GETKEY, "getkey", "-", "the key, or -1 if none is pressed"),
    (SYS_SLEEP, "sleep", "x0 milliseconds", "0"),
    (SYS_BEEP, "beep", "x0 frequency, x1 milliseconds", "0"),
    (SYS_TICK, "tick", "-", "milliseconds since the machine started"),
    (SYS_RANDOM, "random", "-", "a random number 0..2^32"),
    (
        SYS_READFILE,
        "readfile",
        "x0 path, x1 path length (0 = up to a zero byte), x2 buffer, x3 size",
        "bytes read, or -1",
    ),
    (
        SYS_WRITEFILE,
        "writefile",
        "x0 path, x1 path length, x2 data, x3 length, x4 append (1) or replace (0)",
        "0, or -1",
    ),
    (
        SYS_PUT,
        "put",
        "x0 column, x1 row, x2 character, x3 color, x4 background",
        "0",
    ),
    (SYS_CURSOR, "cursor", "x0 column, x1 row", "0"),
    (SYS_CLEAR, "clear", "x0 background color", "0"),
    (SYS_COLOR, "color", "x0 color, x1 background", "0"),
    (SYS_SIZE, "size", "-", "columns * 65536 + rows"),
    (SYS_SPEAK, "speak", "x0 text, x1 length", "1 if the machine can talk"),
];

pub fn name_of(number: u64) -> Option<&'static str> {
    TABLE.iter().find(|(n, ..)| *n == number).map(|(_, name, ..)| *name)
}

/// Where a program's output and input go. The plain runner prints to the
/// process; the debugger captures output into its own pane and reads
/// lines on its status row.
pub trait Io {
    fn write(&mut self, p: &Proc, fd: u64, bytes: &[u8]);
    fn read_line(&mut self, p: &Proc) -> Result<Option<String>, Fault>;
}

/// Straight to the process: the way `./prog` runs.
pub struct Direct;

impl Io for Direct {
    fn write(&mut self, p: &Proc, fd: u64, bytes: &[u8]) {
        if fd == 2 {
            p.write_err(bytes);
        } else {
            p.write_out(bytes);
        }
    }
    fn read_line(&mut self, p: &Proc) -> Result<Option<String>, Fault> {
        p.readline("").map_err(|_| Fault::Interrupted)
    }
}

pub struct ProcSys<'a> {
    pub p: &'a Proc,
    pub io: Box<dyn Io + 'a>,
    /// Keyboard input not yet handed to `read`.
    pending: Vec<u8>,
}

impl<'a> ProcSys<'a> {
    pub fn new(p: &'a Proc, io: Box<dyn Io + 'a>) -> ProcSys<'a> {
        ProcSys {
            p,
            io,
            pending: Vec::new(),
        }
    }

    fn string_arg(vm: &Vm, ptr: u64, len: u64) -> Result<String, Fault> {
        if len == 0 {
            vm.read_cstr(ptr, 4096)
        } else {
            let b = vm.read(ptr, len.min(65536))?;
            Ok(String::from_utf8_lossy(b).into_owned())
        }
    }

    fn interrupted(&self) -> Result<(), Fault> {
        if self.p.killed() {
            Err(Fault::Interrupted)
        } else {
            Ok(())
        }
    }
}

impl Sys for ProcSys<'_> {
    fn syscall(&mut self, vm: &mut Vm, number: u64) -> Result<Option<i32>, Fault> {
        self.interrupted()?;
        let a = [vm.x[0], vm.x[1], vm.x[2], vm.x[3], vm.x[4], vm.x[5]];
        let p = self.p;
        let result: u64 = match number {
            SYS_READ => {
                if a[0] != 0 {
                    return Err(Fault::Sys(format!(
                        "read from fd {}: only fd 0 (the keyboard) can be read here",
                        a[0]
                    )));
                }
                if self.pending.is_empty() {
                    if let Some(line) = self.io.read_line(p)? {
                        self.pending = line.into_bytes();
                        self.pending.push(b'\n');
                    }
                }
                let n = (a[2] as usize).min(self.pending.len());
                if n > 0 {
                    // check the buffer before draining the input
                    vm.write(a[1], &self.pending[..n])?;
                    self.pending.drain(..n);
                }
                n as u64
            }
            SYS_WRITE => {
                let n = a[2].min(1 << 20);
                let bytes = vm.read(a[1], n)?.to_vec();
                self.io.write(p, a[0], &bytes);
                n
            }
            SYS_EXIT => return Ok(Some(a[0] as i32)),
            SYS_NANOSLEEP => {
                let secs = vm.read_u(a[0], 3)? as i64;
                let nanos = vm.read_u(a[0] + 8, 3)? as i64;
                let ms = secs.max(0) as u64 * 1000 + (nanos.max(0) as u64) / 1_000_000;
                p.sleep(ms).map_err(|_| Fault::Interrupted)?;
                0
            }
            SYS_GETRANDOM => {
                let n = a[1].min(65536) as usize;
                let mut buf = vec![0u8; n];
                rand::rng().fill_bytes(&mut buf);
                vm.write(a[0], &buf)?;
                n as u64
            }
            SYS_READKEY => p.readkey().map_err(|_| Fault::Interrupted)?.code() as i64 as u64,
            SYS_GETKEY => p.getkey().map(|k| k.code() as i64).unwrap_or(-1) as u64,
            SYS_SLEEP => {
                p.sleep(a[0].min(60_000)).map_err(|_| Fault::Interrupted)?;
                0
            }
            SYS_BEEP => {
                p.beep(a[0].min(20_000) as u32, a[1].min(10_000) as u32);
                0
            }
            SYS_TICK => p.tick(),
            SYS_RANDOM => rand::rng().next_u32() as u64,
            SYS_READFILE => {
                let path = Self::string_arg(vm, a[0], a[1])?;
                match p.fs().read(&path) {
                    Ok(data) => {
                        let n = data.len().min(a[3] as usize);
                        vm.write(a[2], &data[..n])?;
                        n as u64
                    }
                    Err(_) => -1i64 as u64,
                }
            }
            SYS_WRITEFILE => {
                let path = Self::string_arg(vm, a[0], a[1])?;
                let data = vm.read(a[2], a[3].min(1 << 20))?.to_vec();
                let r = if a[4] != 0 {
                    p.fs().append(&path, &data)
                } else {
                    p.fs().write(&path, &data)
                };
                if r.is_ok() {
                    0
                } else {
                    -1i64 as u64
                }
            }
            SYS_PUT => {
                let ch = char::from_u32(a[2] as u32).unwrap_or('?');
                p.put(
                    a[0].min(65535) as u16,
                    a[1].min(65535) as u16,
                    ch,
                    (a[3] & 15) as u8,
                    (a[4] & 15) as u8,
                );
                0
            }
            SYS_CURSOR => {
                p.cursor(a[0].min(65535) as u16, a[1].min(65535) as u16);
                0
            }
            SYS_CLEAR => {
                p.clear((a[0] & 15) as u8);
                0
            }
            SYS_COLOR => {
                p.set_color((a[0] & 15) as u8, (a[1] & 15) as u8);
                0
            }
            SYS_SIZE => {
                let (c, r) = p.size();
                ((c as u64) << 16) | r as u64
            }
            SYS_SPEAK => {
                let text = Self::string_arg(vm, a[0], a[1])?;
                p.speak(&text) as u64
            }
            n => return Err(Fault::UnknownSyscall { number: n }),
        };
        vm.x[0] = result;
        Ok(None)
    }
}
