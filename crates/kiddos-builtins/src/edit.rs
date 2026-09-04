//! `edit`: a small full-screen text editor in the spirit of nano.
//!
//! Title bar on top, help bar at the bottom, a message line above it.
//! Ctrl-S saves, Ctrl-Q quits (asks if unsaved), Ctrl-K cuts a line,
//! Ctrl-U pastes it back. Nothing to learn before you can write a file.

use crate::util::wants_help;
use kiddos_kernel::{CmdResult, Command, Console, Interrupted, Kernel, Key, Proc, Topic};

pub fn register(k: &Kernel) {
    k.register(Command::new("edit", edit, "write or change a file (a text editor)", Topic::Programs).keep_alive());
}

const TAB: usize = 4;

struct State {
    lines: Vec<Vec<char>>,
    cx: usize,
    cy: usize,
    scroll: usize,
    col_scroll: usize,
    path: String,
    dirty: bool,
    clipboard: Vec<Vec<char>>,
    msg: String,
}

impl State {
    fn line_len(&self) -> usize {
        self.lines[self.cy].len()
    }

    fn clamp(&mut self) {
        if self.cy >= self.lines.len() {
            self.cy = self.lines.len() - 1;
        }
        if self.cx > self.line_len() {
            self.cx = self.line_len();
        }
    }

    fn insert(&mut self, c: char) {
        let cx = self.cx;
        self.lines[self.cy].insert(cx, c);
        self.cx += 1;
        self.dirty = true;
    }

    fn newline(&mut self) {
        let rest: Vec<char> = self.lines[self.cy].split_off(self.cx);
        // keep the indentation of the line above
        let indent: Vec<char> = self.lines[self.cy].iter().take_while(|c| **c == ' ').copied().collect();
        let n = indent.len();
        let mut new = indent;
        new.extend(rest);
        self.lines.insert(self.cy + 1, new);
        self.cy += 1;
        self.cx = n;
        self.dirty = true;
    }

    fn backspace(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
            let cx = self.cx;
            self.lines[self.cy].remove(cx);
            self.dirty = true;
        } else if self.cy > 0 {
            let line = self.lines.remove(self.cy);
            self.cy -= 1;
            self.cx = self.line_len();
            self.lines[self.cy].extend(line);
            self.dirty = true;
        }
    }

    fn delete(&mut self) {
        if self.cx < self.line_len() {
            let cx = self.cx;
            self.lines[self.cy].remove(cx);
            self.dirty = true;
        } else if self.cy + 1 < self.lines.len() {
            let line = self.lines.remove(self.cy + 1);
            self.lines[self.cy].extend(line);
            self.dirty = true;
        }
    }

    fn cut_line(&mut self, chain: bool) {
        if !chain {
            self.clipboard.clear();
        }
        if self.lines.len() == 1 {
            self.clipboard.push(std::mem::take(&mut self.lines[0]));
        } else {
            self.clipboard.push(self.lines.remove(self.cy));
        }
        self.cx = 0;
        self.dirty = true;
        self.clamp();
    }

    fn paste(&mut self) {
        if self.clipboard.is_empty() {
            self.msg = "Nothing to paste. Ctrl-K cuts a line first.".into();
            return;
        }
        for (i, l) in self.clipboard.clone().into_iter().enumerate() {
            self.lines.insert(self.cy + i, l);
        }
        self.cy += self.clipboard.len();
        if self.cy >= self.lines.len() {
            self.lines.push(Vec::new());
        }
        self.cx = 0;
        self.dirty = true;
    }

    fn text(&self) -> String {
        let mut s: String = self
            .lines
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        s.push('\n');
        s
    }
}

fn edit(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    if !p.stdout_is_tty() || !p.stdin_is_tty() {
        p.eprintln("edit: I need the screen and the keyboard for this.");
        return Ok(1);
    }
    let Some(path) = args.first() else {
        p.println(&p.t("usage", &[("usage", "edit <file>   (a new file is fine)")]));
        return Ok(1);
    };
    let lines: Vec<Vec<char>> = match p.fs().stat(path) {
        Ok(st) if st.is_dir() => {
            p.eprintln(&format!("edit: {}", p.t("is-dir", &[("path", path)])));
            return Ok(1);
        }
        Ok(_) => match p.fs().read_string(path) {
            Ok(s) => {
                let mut v: Vec<Vec<char>> = s.lines().map(|l| l.chars().collect()).collect();
                if v.is_empty() {
                    v.push(Vec::new());
                }
                v
            }
            Err(e) => {
                p.complain(&e);
                return Ok(1);
            }
        },
        Err(_) => vec![Vec::new()],
    };
    let mut st = State {
        lines,
        cx: 0,
        cy: 0,
        scroll: 0,
        col_scroll: 0,
        path: path.clone(),
        dirty: false,
        clipboard: Vec::new(),
        msg: "Ctrl-S saves, Ctrl-Q quits.".into(),
    };
    let result = run(p, &mut st);
    // leave the screen clean
    p.print("\x1b[0m\x1b[2J\x1b[H");
    p.cursor_show(true);
    result
}

fn draw(p: &Proc, st: &mut State) {
    let (cols, rows) = p.size();
    let (cols, rows) = (cols as usize, rows as usize);
    let text_rows = rows.saturating_sub(3).max(1);
    // keep the cursor on screen
    if st.cy < st.scroll {
        st.scroll = st.cy;
    }
    if st.cy >= st.scroll + text_rows {
        st.scroll = st.cy + 1 - text_rows;
    }
    if st.cx < st.col_scroll {
        st.col_scroll = st.cx;
    }
    if st.cx >= st.col_scroll + cols {
        st.col_scroll = st.cx + 1 - cols;
    }
    let mut out = String::with_capacity(cols * rows + 64);
    let title = format!(" edit: {}{}", st.path, if st.dirty { "  [changed]" } else { "" });
    out.push_str(&format!("\x1b[H\x1b[7m{:<w$}\x1b[0m", truncate(&title, cols), w = cols));
    for r in 0..text_rows {
        let y = st.scroll + r;
        out.push_str(&format!("\x1b[{};1H", r + 2));
        if let Some(line) = st.lines.get(y) {
            let visible: String = line
                .iter()
                .skip(st.col_scroll)
                .take(cols)
                .map(|c| if *c == '\t' { ' ' } else { *c })
                .collect();
            out.push_str(&visible);
        } else {
            out.push_str("\x1b[34m~\x1b[0m");
        }
        out.push_str("\x1b[K");
    }
    out.push_str(&format!(
        "\x1b[{};1H\x1b[36m{}\x1b[0m\x1b[K",
        rows - 1,
        truncate(&st.msg, cols)
    ));
    let help = " ^S Save   ^Q Quit   ^K Cut line   ^U Paste   ^A/^E Line start/end ";
    out.push_str(&format!(
        "\x1b[{};1H\x1b[7m{:<w$}\x1b[0m",
        rows,
        truncate(help, cols),
        w = cols
    ));
    p.print(&out);
    p.cursor((st.cx - st.col_scroll) as u16, (st.cy - st.scroll + 1) as u16);
    p.cursor_show(true);
}

fn truncate(s: &str, w: usize) -> String {
    s.chars().take(w).collect()
}

fn save(p: &Proc, st: &mut State) -> bool {
    match p.fs().write(&st.path, st.text().as_bytes()) {
        Ok(()) => {
            st.dirty = false;
            st.msg = format!("Saved {} lines to {}.", st.lines.len(), st.path);
            true
        }
        Err(e) => {
            st.msg = format!("Can't save: {}", p.explain(&e));
            false
        }
    }
}

fn run(p: &Proc, st: &mut State) -> Result<i32, Interrupted> {
    let mut last_cut = false;
    loop {
        draw(p, st);
        let key = p.readkey()?;
        let mut cut = false;
        match key {
            Key::Char(c) => st.insert(c),
            Key::Tab => {
                for _ in 0..TAB {
                    st.insert(' ');
                }
            }
            Key::Enter => st.newline(),
            Key::Backspace => st.backspace(),
            Key::Delete => st.delete(),
            Key::Left => {
                if st.cx > 0 {
                    st.cx -= 1;
                } else if st.cy > 0 {
                    st.cy -= 1;
                    st.cx = st.line_len();
                }
            }
            Key::Right => {
                if st.cx < st.line_len() {
                    st.cx += 1;
                } else if st.cy + 1 < st.lines.len() {
                    st.cy += 1;
                    st.cx = 0;
                }
            }
            Key::Up => {
                st.cy = st.cy.saturating_sub(1);
                st.clamp();
            }
            Key::Down => {
                if st.cy + 1 < st.lines.len() {
                    st.cy += 1;
                }
                st.clamp();
            }
            Key::PageUp => {
                let (_, rows) = p.size();
                st.cy = st.cy.saturating_sub(rows as usize - 3);
                st.clamp();
            }
            Key::PageDown => {
                let (_, rows) = p.size();
                st.cy = (st.cy + rows as usize - 3).min(st.lines.len() - 1);
                st.clamp();
            }
            Key::Home | Key::Ctrl('a') => st.cx = 0,
            Key::End | Key::Ctrl('e') => st.cx = st.line_len(),
            Key::Ctrl('k') => {
                st.cut_line(last_cut);
                cut = true;
                st.msg = format!("Cut {} line(s). Ctrl-U pastes.", st.clipboard.len());
            }
            Key::Ctrl('u') => st.paste(),
            Key::Ctrl('s') => {
                save(p, st);
            }
            Key::Ctrl('q') | Key::Ctrl('c') | Key::Escape => {
                if !st.dirty {
                    return Ok(0);
                }
                st.msg = "Save changes? y = yes, n = no, Esc = keep editing".into();
                draw(p, st);
                loop {
                    match p.readkey()? {
                        Key::Char('y') | Key::Char('Y') => {
                            if save(p, st) {
                                return Ok(0);
                            }
                            break;
                        }
                        Key::Char('n') | Key::Char('N') => return Ok(0),
                        Key::Escape | Key::Ctrl('c') | Key::Ctrl('q') => {
                            st.msg = "OK, still editing.".into();
                            break;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        last_cut = cut;
    }
}
