//! The `kiddos` import module: the console API as host functions.
//!
//! Strings are (pointer, length) into the module's memory. Keys are
//! integers: a Unicode code point for printable keys, `0x110000 + n` for
//! named keys, `0x120000 + letter` for Ctrl, `0x130000 + letter` for Alt.
//! `/usr/include/kiddos.h` mirrors these numbers.

use kiddos_kernel::{Console, Key, KeyEvent, Proc};
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

/// A key event as one integer: the key code, plus this bit when released.
pub const KEY_UP_BIT: i32 = 0x100_0000;

pub fn eventcode(e: KeyEvent) -> i32 {
    keycode(e.key) | if e.down { 0 } else { KEY_UP_BIT }
}

/// The inverse of [`keycode`], for `key_down(code)`.
pub fn key_from_code(code: i32) -> Option<Key> {
    Some(match code {
        c if (0..KEY_NAMED).contains(&c) => Key::Char(char::from_u32(c as u32)?),
        c if (KEY_CTRL..KEY_ALT).contains(&c) => Key::Ctrl(char::from_u32((c - KEY_CTRL) as u32)?),
        c if (KEY_ALT..KEY_ALT + 0x10000).contains(&c) => Key::Alt(char::from_u32((c - KEY_ALT) as u32)?),
        c => match c - KEY_NAMED {
            1 => Key::Enter,
            2 => Key::Backspace,
            3 => Key::Tab,
            4 => Key::Escape,
            5 => Key::Up,
            6 => Key::Down,
            7 => Key::Left,
            8 => Key::Right,
            9 => Key::Home,
            10 => Key::End,
            11 => Key::PageUp,
            12 => Key::PageDown,
            13 => Key::Insert,
            14 => Key::Delete,
            15 => Key::BackTab,
            n if (21..=32).contains(&n) => Key::F((n - 20) as u8),
            _ => return None,
        },
    })
}

fn read_bytes(caller: &mut Caller<'_, State>, ptr: i32, len: i32) -> anyhow::Result<Vec<u8>> {
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
    Ok(data[start..end].to_vec())
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

    // ---- pixel mode (API v2) ------------------------------------------
    l.func_wrap(m, "gfx_mode", |c: Caller<'_, State>, on: i32| {
        c.data().proc.gfx_mode(on != 0);
    })?;
    l.func_wrap(m, "gfx_clear", |c: Caller<'_, State>, color: i32| {
        c.data().proc.gfx_clear(color as u8);
    })?;
    l.func_wrap(m, "gfx_pixel", |c: Caller<'_, State>, x: i32, y: i32, color: i32| {
        c.data().proc.gfx_pixel(x, y, color as u8);
    })?;
    l.func_wrap(m, "gfx_get", |c: Caller<'_, State>, x: i32, y: i32| -> i32 {
        c.data().proc.gfx_get(x, y) as i32
    })?;
    l.func_wrap(
        m,
        "gfx_line",
        |c: Caller<'_, State>, x1: i32, y1: i32, x2: i32, y2: i32, color: i32| {
            c.data().proc.gfx_line(x1, y1, x2, y2, color as u8);
        },
    )?;
    l.func_wrap(
        m,
        "gfx_rect",
        |c: Caller<'_, State>, x: i32, y: i32, w: i32, h: i32, color: i32| {
            c.data().proc.gfx_rect(x, y, w, h, color as u8);
        },
    )?;
    l.func_wrap(
        m,
        "gfx_fill",
        |c: Caller<'_, State>, x: i32, y: i32, w: i32, h: i32, color: i32| {
            c.data().proc.gfx_fill(x, y, w, h, color as u8);
        },
    )?;
    l.func_wrap(
        m,
        "gfx_circle",
        |c: Caller<'_, State>, x: i32, y: i32, r: i32, color: i32, filled: i32| {
            c.data().proc.gfx_circle(x, y, r, color as u8, filled != 0);
        },
    )?;
    l.func_wrap(
        m,
        "gfx_blit",
        |mut c: Caller<'_, State>, x: i32, y: i32, w: i32, h: i32, ptr: i32, transparent: i32| -> anyhow::Result<()> {
            let (w, h) = (w.clamp(0, 4096), h.clamp(0, 4096));
            let data = read_bytes(&mut c, ptr, w * h)?;
            let t = if (0..256).contains(&transparent) {
                Some(transparent as u8)
            } else {
                None
            };
            c.data().proc.gfx_blit(x, y, w, h, &data, t);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "gfx_read",
        |mut c: Caller<'_, State>, x: i32, y: i32, w: i32, h: i32, ptr: i32| -> anyhow::Result<i32> {
            let (w, h) = (w.clamp(0, 4096), h.clamp(0, 4096));
            let data = c.data().proc.gfx_read(x, y, w, h);
            write_bytes(&mut c, ptr, w * h, &data)
        },
    )?;
    l.func_wrap(
        m,
        "gfx_palette",
        |c: Caller<'_, State>, i: i32, r: i32, g: i32, b: i32| {
            c.data().proc.gfx_palette(
                i as u8,
                [r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8],
            );
        },
    )?;
    l.func_wrap(
        m,
        "gfx_text",
        |mut c: Caller<'_, State>, x: i32, y: i32, ptr: i32, len: i32, fg: i32, bg: i32| -> anyhow::Result<i32> {
            let s = read_str(&mut c, ptr, len)?;
            let bg = if (0..256).contains(&bg) { Some(bg as u8) } else { None };
            Ok(c.data().proc.gfx_text(x, y, &s, fg as u8, bg))
        },
    )?;
    l.func_wrap(m, "gfx_flip", |c: Caller<'_, State>| {
        c.data().proc.gfx_flip();
    })?;

    // ---- key state (API v2) --------------------------------------------
    l.func_wrap(m, "key_down", |c: Caller<'_, State>, code: i32| -> i32 {
        key_from_code(code).map(|k| c.data().proc.key_held(k)).unwrap_or(false) as i32
    })?;
    l.func_wrap(m, "key_event", |c: Caller<'_, State>| -> i32 {
        c.data().proc.key_event().map(eventcode).unwrap_or(-1)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_round_trip() {
        for k in [
            Key::Char('a'),
            Key::Char('Ж'),
            Key::Enter,
            Key::Escape,
            Key::Left,
            Key::BackTab,
            Key::F(1),
            Key::F(12),
            Key::Ctrl('c'),
            Key::Alt('x'),
        ] {
            assert_eq!(key_from_code(keycode(k)), Some(k), "{k:?}");
        }
        assert_eq!(key_from_code(-1), None);
        assert_eq!(
            eventcode(KeyEvent {
                key: Key::Up,
                down: false
            }),
            keycode(Key::Up) | KEY_UP_BIT
        );
    }
}
