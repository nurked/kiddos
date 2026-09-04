//! The `kiddos` import module: the console API as host functions.
//!
//! Strings are (pointer, length) into the module's memory. Keys are
//! integers: a Unicode code point for printable keys, `0x110000 + n` for
//! named keys, `0x120000 + letter` for Ctrl, `0x130000 + letter` for Alt.
//! `/usr/include/kiddos.h` mirrors these numbers.

use kiddos_kernel::{Console, Key, Proc};
use std::sync::Arc;
use wasmtime::{Caller, Linker};

pub struct State {
    pub proc: Arc<Proc>,
    pub limits: wasmtime::StoreLimits,
    rng: u64,
}

impl State {
    pub fn new(proc: Arc<Proc>) -> State {
        let seed = proc.tick() ^ 0x9E37_79B9_7F4A_7C15 ^ ((proc.pid as u64) << 32);
        State {
            proc,
            limits: wasmtime::StoreLimitsBuilder::new()
                .memory_size(crate::MEMORY_LIMIT)
                .instances(1)
                .tables(4)
                .build(),
            rng: if seed == 0 { 1 } else { seed },
        }
    }
}

/// A host function asked to end the program.
#[derive(Debug)]
pub struct Exit(pub i32);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {}", self.0)
    }
}
impl std::error::Error for Exit {}

pub const KEY_NAMED: i32 = 0x110000;
pub const KEY_CTRL: i32 = 0x120000;
pub const KEY_ALT: i32 = 0x130000;

pub fn keycode(k: Key) -> i32 {
    match k {
        Key::Char(c) => c as i32,
        Key::Enter => KEY_NAMED + 1,
        Key::Backspace => KEY_NAMED + 2,
        Key::Tab => KEY_NAMED + 3,
        Key::Escape => KEY_NAMED + 4,
        Key::Up => KEY_NAMED + 5,
        Key::Down => KEY_NAMED + 6,
        Key::Left => KEY_NAMED + 7,
        Key::Right => KEY_NAMED + 8,
        Key::Home => KEY_NAMED + 9,
        Key::End => KEY_NAMED + 10,
        Key::PageUp => KEY_NAMED + 11,
        Key::PageDown => KEY_NAMED + 12,
        Key::Insert => KEY_NAMED + 13,
        Key::Delete => KEY_NAMED + 14,
        Key::BackTab => KEY_NAMED + 15,
        Key::F(n) => KEY_NAMED + 20 + n as i32,
        Key::Ctrl(c) => KEY_CTRL + c as i32,
        Key::Alt(c) => KEY_ALT + c as i32,
    }
}

fn read_str(caller: &mut Caller<'_, State>, ptr: i32, len: i32) -> anyhow::Result<String> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("The program has no memory export."))?;
    let (start, len) = (ptr.max(0) as usize, len.max(0) as usize);
    let data = mem.data(&*caller);
    let end = start
        .checked_add(len)
        .filter(|e| *e <= data.len())
        .ok_or(wasmtime::Trap::MemoryOutOfBounds)?;
    Ok(String::from_utf8_lossy(&data[start..end]).to_string())
}

fn write_bytes(caller: &mut Caller<'_, State>, ptr: i32, cap: i32, bytes: &[u8]) -> anyhow::Result<i32> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow::anyhow!("The program has no memory export."))?;
    let n = bytes.len().min(cap.max(0) as usize);
    mem.write(&mut *caller, ptr.max(0) as usize, &bytes[..n])
        .map_err(|_| wasmtime::Trap::MemoryOutOfBounds)?;
    Ok(n as i32)
}

fn interrupted() -> anyhow::Error {
    wasmtime::Trap::Interrupt.into()
}

pub fn link(l: &mut Linker<State>) -> anyhow::Result<()> {
    let m = crate::MODULE;
    l.func_wrap(
        m,
        "print",
        |mut c: Caller<'_, State>, ptr: i32, len: i32| -> anyhow::Result<()> {
            let s = read_str(&mut c, ptr, len)?;
            c.data().proc.print(&s);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "eprint",
        |mut c: Caller<'_, State>, ptr: i32, len: i32| -> anyhow::Result<()> {
            let s = read_str(&mut c, ptr, len)?;
            c.data().proc.eprint(&s);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "put",
        |c: Caller<'_, State>, x: i32, y: i32, ch: i32, fg: i32, bg: i32| {
            let ch = char::from_u32(ch.max(0) as u32).unwrap_or('?');
            c.data()
                .proc
                .put(x.max(0) as u16, y.max(0) as u16, ch, fg as u8, bg as u8);
        },
    )?;
    l.func_wrap(m, "cursor", |c: Caller<'_, State>, x: i32, y: i32| {
        c.data().proc.cursor(x.max(0) as u16, y.max(0) as u16);
    })?;
    l.func_wrap(m, "cursor_show", |c: Caller<'_, State>, v: i32| {
        c.data().proc.cursor_show(v != 0);
    })?;
    l.func_wrap(m, "clear", |c: Caller<'_, State>, bg: i32| {
        c.data().proc.clear(bg as u8);
    })?;
    l.func_wrap(m, "color", |c: Caller<'_, State>, fg: i32, bg: i32| {
        c.data().proc.set_color(fg as u8, bg as u8);
    })?;
    l.func_wrap(m, "size", |c: Caller<'_, State>| -> i32 {
        let (cols, rows) = c.data().proc.size();
        ((cols as i32) << 16) | rows as i32
    })?;
    l.func_wrap(m, "getkey", |c: Caller<'_, State>| -> i32 {
        c.data().proc.getkey().map(keycode).unwrap_or(-1)
    })?;
    l.func_wrap(m, "readkey", |c: Caller<'_, State>| -> anyhow::Result<i32> {
        c.data().proc.readkey().map(keycode).map_err(|_| interrupted())
    })?;
    l.func_wrap(
        m,
        "readline",
        |mut c: Caller<'_, State>, ptr: i32, cap: i32| -> anyhow::Result<i32> {
            let line = c.data().proc.readline("").map_err(|_| interrupted())?;
            match line {
                Some(s) => write_bytes(&mut c, ptr, cap, s.as_bytes()),
                None => Ok(-1),
            }
        },
    )?;
    l.func_wrap(m, "sleep", |c: Caller<'_, State>, ms: i32| -> anyhow::Result<()> {
        c.data().proc.sleep(ms.max(0) as u64).map_err(|_| interrupted())
    })?;
    l.func_wrap(m, "tick", |c: Caller<'_, State>| -> i64 { c.data().proc.tick() as i64 })?;
    l.func_wrap(m, "beep", |c: Caller<'_, State>, freq: i32, ms: i32| {
        c.data().proc.beep(freq.max(0) as u32, ms.max(0) as u32);
    })?;
    l.func_wrap(
        m,
        "speak",
        |mut c: Caller<'_, State>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let s = read_str(&mut c, ptr, len)?;
            Ok(c.data().proc.speak(&s) as i32)
        },
    )?;
    l.func_wrap(m, "random", |mut c: Caller<'_, State>| -> i32 {
        let s = c.data_mut();
        s.rng ^= s.rng << 13;
        s.rng ^= s.rng >> 7;
        s.rng ^= s.rng << 17;
        (s.rng >> 33) as i32 & 0x7FFF_FFFF
    })?;
    l.func_wrap(m, "exit", |_c: Caller<'_, State>, code: i32| -> anyhow::Result<()> {
        Err(Exit(code).into())
    })?;
    l.func_wrap(
        m,
        "fs_read",
        |mut c: Caller<'_, State>, pp: i32, pl: i32, buf: i32, cap: i32| -> anyhow::Result<i32> {
            let path = read_str(&mut c, pp, pl)?;
            match c.data().proc.fs().read(&path) {
                Ok(data) => write_bytes(&mut c, buf, cap, &data),
                Err(_) => Ok(-1),
            }
        },
    )?;
    l.func_wrap(
        m,
        "fs_write",
        |mut c: Caller<'_, State>, pp: i32, pl: i32, dp: i32, dl: i32, append: i32| -> anyhow::Result<i32> {
            let path = read_str(&mut c, pp, pl)?;
            let data = read_str(&mut c, dp, dl)?;
            let r = if append != 0 {
                c.data().proc.fs().append(&path, data.as_bytes())
            } else {
                c.data().proc.fs().write(&path, data.as_bytes())
            };
            Ok(if r.is_ok() { 0 } else { -1 })
        },
    )?;
    Ok(())
}
