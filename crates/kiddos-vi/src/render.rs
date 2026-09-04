//! Draw an engine's state into a band of screen rows.

use crate::engine::{Mode, Vi};
use kiddos_kernel::{Console, Proc};

/// Draw `vi` in rows `top .. top+rows` (the last of them is the status
/// line). With `walls`, `#` is drawn as stone and `X` as the exit.
pub fn draw(p: &Proc, vi: &mut Vi, top: u16, rows: u16, walls: bool) {
    let (cols, _) = p.size();
    let cols = cols as usize;
    let text_rows = rows.saturating_sub(1).max(1) as usize;
    if vi.cy < vi.scroll {
        vi.scroll = vi.cy;
    }
    if vi.cy >= vi.scroll + text_rows {
        vi.scroll = vi.cy + 1 - text_rows;
    }
    let mut out = String::with_capacity(cols * rows as usize + 64);
    for r in 0..text_rows {
        let y = vi.scroll + r;
        out.push_str(&format!("\x1b[{};1H", top as usize + r + 1));
        match vi.lines.get(y) {
            Some(line) => {
                let mut shown = 0;
                for c in line.iter().take(cols) {
                    if walls && *c == '#' {
                        out.push_str("\x1b[33m#\x1b[0m");
                    } else if walls && *c == 'X' {
                        out.push_str("\x1b[1;33mX\x1b[0m");
                    } else if walls && *c == 'o' {
                        out.push_str("\x1b[31mo\x1b[0m");
                    } else {
                        out.push(*c);
                    }
                    shown += 1;
                }
                let _ = shown;
            }
            None => out.push_str("\x1b[1;34m~\x1b[0m"),
        }
        out.push_str("\x1b[K");
    }
    let status = vi.status_line();
    let status: String = status.chars().take(cols).collect();
    let styled = if vi.mode == Mode::Insert {
        format!("\x1b[1m{status}\x1b[0m")
    } else if status.starts_with('E') && status.contains(':') {
        format!("\x1b[1;31m{status}\x1b[0m")
    } else {
        status
    };
    out.push_str(&format!("\x1b[{};1H{styled}\x1b[K", top as usize + rows as usize));
    p.print(&out);
    match vi.mode {
        Mode::Command | Mode::Search => {
            p.cursor((vi.cmdline.chars().count() + 1).min(cols - 1) as u16, top + rows - 1);
        }
        _ => {
            p.cursor(vi.cx.min(cols - 1) as u16, top + (vi.cy - vi.scroll) as u16);
        }
    }
    p.cursor_show(true);
}
