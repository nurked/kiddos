//! The line editor: cursor movement, history, tab completion.

use kiddos_kernel::{Console, Interrupted, Key, Proc};

pub const MAX_HISTORY: usize = 500;

pub enum ReadOutcome {
    Line(String),
    /// Ctrl-D on an empty line.
    Eof,
    /// Ctrl-C: the line was thrown away.
    Cancelled,
}

#[derive(Default)]
pub struct Editor {
    history: Vec<String>,
}

struct View {
    x0: u16,
    y0: u16,
    cols: u16,
    rows: u16,
    used_rows: u16,
}

impl Editor {
    pub fn new() -> Editor {
        Editor::default()
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn load_history(&mut self, lines: impl IntoIterator<Item = String>) {
        for l in lines {
            self.push_history(&l);
        }
    }

    pub fn push_history(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() || self.history.last().map(|l| l == line).unwrap_or(false) {
            return;
        }
        self.history.push(line.to_string());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
    }

    fn redraw(p: &Proc, v: &mut View, buf: &[char], pos: usize) {
        p.cursor(v.x0, v.y0);
        let s: String = buf.iter().collect();
        p.print(&s);
        p.print("\x1b[K");
        let len = buf.len() as u16;
        let l = v.x0 + len;
        let (_, ey) = p.cursor_pos();
        v.y0 = if len == 0 {
            ey
        } else {
            ey.saturating_sub((l - 1) / v.cols)
        };
        let used = l.div_ceil(v.cols).max(1);
        for r in used..v.used_rows {
            if v.y0 + r < v.rows {
                p.cursor(0, v.y0 + r);
                p.print("\x1b[K");
            }
        }
        v.used_rows = used;
        let pp = v.x0 + pos as u16;
        p.cursor(pp % v.cols, v.y0 + pp / v.cols);
    }

    fn show_prompt(p: &Proc, prompt: &str) -> View {
        p.print(prompt);
        let (x0, y0) = p.cursor_pos();
        let (cols, rows) = p.size();
        View {
            x0,
            y0,
            cols,
            rows,
            used_rows: 1,
        }
    }

    /// Read one line interactively.
    pub fn read_line(&mut self, p: &Proc, prompt: &str) -> Result<ReadOutcome, Interrupted> {
        let mut v = Self::show_prompt(p, prompt);
        let mut buf: Vec<char> = Vec::new();
        let mut pos = 0usize;
        let mut hist_idx = self.history.len();
        let mut saved: Vec<char> = Vec::new();
        let max_len = (v.cols as usize * 3).saturating_sub(v.x0 as usize + 1);
        let mut last_tab_no_progress = false;
        p.cursor_show(true);
        loop {
            let key = p.readkey()?;
            let mut tab = false;
            match key {
                Key::Enter => {
                    let pp = v.x0 + buf.len() as u16;
                    p.cursor(pp % v.cols, v.y0 + pp / v.cols);
                    p.print("\n");
                    return Ok(ReadOutcome::Line(buf.iter().collect()));
                }
                Key::Ctrl('c') => {
                    p.print("^C\n");
                    return Ok(ReadOutcome::Cancelled);
                }
                Key::Ctrl('d') if buf.is_empty() => {
                    p.print("\n");
                    return Ok(ReadOutcome::Eof);
                }
                Key::Ctrl('l') => {
                    p.clear(0);
                    v = Self::show_prompt(p, prompt);
                }
                Key::Char(c) => {
                    if buf.len() < max_len {
                        buf.insert(pos, c);
                        pos += 1;
                    }
                }
                Key::Backspace => {
                    if pos > 0 {
                        pos -= 1;
                        buf.remove(pos);
                    }
                }
                Key::Delete | Key::Ctrl('d') => {
                    if pos < buf.len() {
                        buf.remove(pos);
                    }
                }
                Key::Left | Key::Ctrl('b') => pos = pos.saturating_sub(1),
                Key::Right | Key::Ctrl('f') => pos = (pos + 1).min(buf.len()),
                Key::Home | Key::Ctrl('a') => pos = 0,
                Key::End | Key::Ctrl('e') => pos = buf.len(),
                Key::Ctrl('u') => {
                    buf.drain(..pos);
                    pos = 0;
                }
                Key::Ctrl('k') => buf.truncate(pos),
                Key::Ctrl('w') => {
                    let mut i = pos;
                    while i > 0 && buf[i - 1] == ' ' {
                        i -= 1;
                    }
                    while i > 0 && buf[i - 1] != ' ' {
                        i -= 1;
                    }
                    buf.drain(i..pos);
                    pos = i;
                }
                Key::Up | Key::Ctrl('p') => {
                    if hist_idx > 0 {
                        if hist_idx == self.history.len() {
                            saved = buf.clone();
                        }
                        hist_idx -= 1;
                        buf = self.history[hist_idx].chars().collect();
                        pos = buf.len();
                    }
                }
                Key::Down | Key::Ctrl('n') => {
                    if hist_idx < self.history.len() {
                        hist_idx += 1;
                        buf = if hist_idx == self.history.len() {
                            saved.clone()
                        } else {
                            self.history[hist_idx].chars().collect()
                        };
                        pos = buf.len();
                    }
                }
                Key::Tab => {
                    tab = true;
                    let progressed = self.complete(p, &mut buf, &mut pos, last_tab_no_progress, prompt, &mut v);
                    last_tab_no_progress = !progressed;
                }
                _ => {}
            }
            if !tab {
                last_tab_no_progress = false;
            }
            Self::redraw(p, &mut v, &buf, pos);
        }
    }

    /// Returns true if the buffer changed.
    fn complete(
        &self,
        p: &Proc,
        buf: &mut Vec<char>,
        pos: &mut usize,
        show_list: bool,
        prompt: &str,
        v: &mut View,
    ) -> bool {
        let before: String = buf[..*pos].iter().collect();
        let start = before.rfind(' ').map(|i| i + 1).unwrap_or(0);
        let word = &before[start..];
        let head = before[..start].trim_end();
        let first = head.is_empty()
            || head.ends_with('|')
            || head.ends_with(';')
            || head.ends_with("&&")
            || head.ends_with("||");
        let (candidates, replace_from): (Vec<String>, usize) = if first && !word.contains('/') {
            let mut names: Vec<String> = p
                .kernel()
                .commands()
                .into_iter()
                .filter(|c| c.topic != kiddos_kernel::Topic::Hidden && (!c.parent_only || p.is_root()))
                .map(|c| c.name.to_string())
                .collect();
            for b in ["cd", "exit", "export", "unset", "history"] {
                names.push(b.to_string());
            }
            if let Ok(entries) = p.fs().readdir("~/bin") {
                names.extend(entries.into_iter().map(|e| e.name));
            }
            names.sort();
            names.dedup();
            let c: Vec<String> = names
                .into_iter()
                .filter(|n| n.starts_with(word))
                .map(|n| format!("{n} "))
                .collect();
            (c, start)
        } else {
            let (dir, prefix) = match word.rfind('/') {
                Some(i) => (&word[..=i], &word[i + 1..]),
                None => ("", word),
            };
            let list_dir = if dir.is_empty() { "." } else { dir };
            let entries = p.fs().readdir(list_dir).unwrap_or_default();
            let mut c: Vec<String> = entries
                .into_iter()
                .filter(|e| e.name.starts_with(prefix) && (prefix.starts_with('.') || !e.name.starts_with('.')))
                .map(|e| {
                    let is_dir = e.is_dir() || (e.is_symlink() && p.fs().is_dir(&format!("{}{}", dir, e.name)));
                    if is_dir {
                        format!("{}/", e.name)
                    } else {
                        format!("{} ", e.name)
                    }
                })
                .collect();
            c.sort();
            (c, start + dir.len())
        };
        if candidates.is_empty() {
            return false;
        }
        let typed_len = *pos - replace_from;
        let insert: String = if candidates.len() == 1 {
            candidates[0].clone()
        } else {
            let mut common = candidates[0].trim_end().to_string();
            for c in &candidates[1..] {
                let c = c.trim_end();
                let n = common.chars().zip(c.chars()).take_while(|(a, b)| a == b).count();
                common = common.chars().take(n).collect();
            }
            common
        };
        let progressed = insert.chars().count() > typed_len;
        if progressed {
            let new_chars: Vec<char> = insert.chars().collect();
            buf.splice(replace_from..*pos, new_chars.iter().copied());
            *pos = replace_from + new_chars.len();
        } else if show_list && candidates.len() > 1 {
            // print the options below, then a fresh prompt
            let pp = v.x0 + buf.len() as u16;
            p.cursor(pp % v.cols, v.y0 + pp / v.cols);
            p.print("\n");
            let names: Vec<String> = candidates.iter().map(|c| c.trim_end().to_string()).collect();
            let width = names.iter().map(|n| n.chars().count()).max().unwrap_or(1) + 2;
            let per_row = ((v.cols as usize) / width).max(1);
            for chunk in names.chunks(per_row) {
                let line: String = chunk.iter().map(|n| format!("{:<w$}", n, w = width)).collect();
                p.print(line.trim_end());
                p.print("\n");
            }
            *v = Self::show_prompt(p, prompt);
        }
        progressed
    }
}
