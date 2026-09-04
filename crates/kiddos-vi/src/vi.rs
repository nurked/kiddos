//! `vi [file]`: the editor, earned by finishing vi-quest.

use crate::engine::{Event, Mode, Vi};
use crate::render;
use kiddos_kernel::{CmdResult, Console, Interrupted, Key, Proc};

pub fn cmd_vi(p: &Proc, args: &[String]) -> CmdResult {
    if !p.stdout_is_tty() || !p.stdin_is_tty() {
        p.eprintln("vi: I need the screen and the keyboard for this.");
        return Ok(1);
    }
    let file = args.first().filter(|a| !a.starts_with('-')).cloned();
    let text = match &file {
        Some(f) => match p.fs().stat(f) {
            Ok(st) if st.is_dir() => {
                p.eprintln(&format!("vi: {}", p.t("is-dir", &[("path", f)])));
                return Ok(1);
            }
            Ok(_) => match p.fs().read_string(f) {
                Ok(s) => s,
                Err(e) => {
                    p.complain(&e);
                    return Ok(1);
                }
            },
            Err(_) => String::new(),
        },
        None => String::new(),
    };
    let mut vi = Vi::new(&text);
    vi.message = match &file {
        Some(f) if !text.is_empty() => format!("\"{f}\" {}L", vi.lines.len()),
        Some(f) => format!("\"{f}\" [New]"),
        None => "vi: :q quits, :wq saves and quits, i inserts, Esc returns. (man vi)".into(),
    };
    let result = run(p, &mut vi, file.as_deref());
    p.print("\x1b[0m\x1b[2J\x1b[H");
    p.cursor_show(true);
    result
}

fn save(p: &Proc, vi: &mut Vi, file: Option<&str>) -> bool {
    let Some(f) = file else {
        vi.message = "E32: No file name".into();
        return false;
    };
    match p.fs().write(f, vi.text().as_bytes()) {
        Ok(()) => {
            vi.dirty = false;
            vi.message = format!("\"{f}\" {}L written", vi.lines.len());
            true
        }
        Err(e) => {
            vi.message = format!("E212: Can't open file for writing ({})", p.explain(&e));
            false
        }
    }
}

fn run(p: &Proc, vi: &mut Vi, file: Option<&str>) -> Result<i32, Interrupted> {
    let (_, rows) = p.size();
    p.print("\x1b[2J");
    loop {
        render::draw(p, vi, 0, rows, false);
        let k = p.readkey()?;
        if k == Key::Ctrl('c') && vi.mode == Mode::Normal {
            vi.message = "Type  :q!  and press Enter to leave without saving, or  :wq  to save and leave.".into();
            continue;
        }
        match vi.key(k) {
            Event::None => {}
            Event::Quit { force } => {
                if vi.dirty && !force {
                    vi.message = "E37: No write since last change (add ! to override)".into();
                } else {
                    return Ok(0);
                }
            }
            Event::Write => {
                save(p, vi, file);
            }
            Event::WriteQuit => {
                if save(p, vi, file) {
                    return Ok(0);
                }
            }
            Event::Unknown(cmd) => vi.message = format!("E492: Not an editor command: {cmd}"),
        }
    }
}
