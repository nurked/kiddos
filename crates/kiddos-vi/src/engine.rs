//! The modal editing engine. No screen, no files: keys in, events out.
//! `vi` wraps it with a file; the games wrap it with rules.

use kiddos_kernel::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    None,
    /// `:q` (`force` for `:q!`)
    Quit {
        force: bool,
    },
    /// `:w`
    Write,
    /// `:wq`, `:x`, `ZZ`
    WriteQuit,
    /// A `:` command the engine does not know.
    Unknown(String),
}

const UNDO_MAX: usize = 100;

#[derive(Debug, Clone)]
pub struct Vi {
    pub lines: Vec<Vec<char>>,
    pub cx: usize,
    pub cy: usize,
    pub mode: Mode,
    pub cmdline: String,
    pub message: String,
    pub dirty: bool,
    pub scroll: usize,
    pending: String,
    count: Option<usize>,
    yank: Vec<Vec<char>>,
    yank_linewise: bool,
    undo: Vec<(Vec<Vec<char>>, usize, usize)>,
    last_search: Option<String>,
    /// Snapshot taken when entering insert mode, so `u` undoes the whole insert.
    insert_snapshot: Option<(Vec<Vec<char>>, usize, usize)>,
    /// Keys the engine has handled (games count these).
    pub keys_seen: usize,
}

impl Vi {
    pub fn new(text: &str) -> Vi {
        let mut lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        Vi {
            lines,
            cx: 0,
            cy: 0,
            mode: Mode::Normal,
            cmdline: String::new(),
            message: String::new(),
            dirty: false,
            scroll: 0,
            pending: String::new(),
            count: None,
            yank: Vec::new(),
            yank_linewise: false,
            undo: Vec::new(),
            last_search: None,
            insert_snapshot: None,
            keys_seen: 0,
        }
    }

    pub fn text(&self) -> String {
        let mut s: String = self
            .lines
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        s.push('\n');
        s
    }

    pub fn char_at(&self, x: usize, y: usize) -> Option<char> {
        self.lines.get(y).and_then(|l| l.get(x)).copied()
    }

    pub fn current(&self) -> Option<char> {
        self.char_at(self.cx, self.cy)
    }

    fn line_len(&self) -> usize {
        self.lines[self.cy].len()
    }

    /// Keep the cursor legal for the mode.
    fn clamp(&mut self) {
        if self.cy >= self.lines.len() {
            self.cy = self.lines.len() - 1;
        }
        let len = self.line_len();
        let max = if self.mode == Mode::Insert {
            len
        } else {
            len.saturating_sub(1)
        };
        if self.cx > max {
            self.cx = max;
        }
    }

    fn snapshot(&mut self) {
        self.undo.push((self.lines.clone(), self.cx, self.cy));
        if self.undo.len() > UNDO_MAX {
            self.undo.remove(0);
        }
        self.dirty = true;
    }

    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    // ---- motions ---------------------------------------------------------

    fn is_word(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Start of the next word (vi's `w`).
    fn next_word(&mut self) {
        let mut x = self.cx;
        let mut y = self.cy;
        let line = &self.lines[y];
        if x < line.len() {
            let kind = Self::is_word(line[x]);
            while x < line.len() && Self::is_word(line[x]) == kind && !line[x].is_whitespace() {
                x += 1;
            }
        }
        loop {
            let line = &self.lines[y];
            while x < line.len() && line[x].is_whitespace() {
                x += 1;
            }
            if x < line.len() {
                break;
            }
            if y + 1 >= self.lines.len() {
                x = line.len().saturating_sub(1);
                break;
            }
            y += 1;
            x = 0;
        }
        self.cx = x;
        self.cy = y;
    }

    /// Start of the previous word (`b`).
    fn prev_word(&mut self) {
        let (mut x, mut y) = (self.cx, self.cy);
        loop {
            if x == 0 {
                if y == 0 {
                    break;
                }
                y -= 1;
                x = self.lines[y].len();
                if x == 0 {
                    continue;
                }
            }
            x -= 1;
            let line = &self.lines[y];
            while x > 0 && line[x].is_whitespace() {
                x -= 1;
            }
            if line[x].is_whitespace() {
                continue;
            }
            let kind = Self::is_word(line[x]);
            while x > 0 && !line[x - 1].is_whitespace() && Self::is_word(line[x - 1]) == kind {
                x -= 1;
            }
            break;
        }
        self.cx = x;
        self.cy = y;
    }

    /// End of the current/next word (`e`).
    fn word_end(&mut self) {
        let (mut x, mut y) = (self.cx, self.cy);
        loop {
            let line = &self.lines[y];
            if x + 1 < line.len() {
                x += 1;
            } else if y + 1 < self.lines.len() {
                y += 1;
                x = 0;
            } else {
                break;
            }
            let line = &self.lines[y];
            while x < line.len() && line[x].is_whitespace() {
                if x + 1 < line.len() {
                    x += 1;
                } else if y + 1 < self.lines.len() {
                    y += 1;
                    x = 0;
                } else {
                    break;
                }
            }
            let line = &self.lines[y];
            if x < line.len() && !line[x].is_whitespace() {
                let kind = Self::is_word(line[x]);
                while x + 1 < line.len() && !line[x + 1].is_whitespace() && Self::is_word(line[x + 1]) == kind {
                    x += 1;
                }
                break;
            }
            if x + 1 >= line.len() && y + 1 >= self.lines.len() {
                break;
            }
        }
        self.cx = x;
        self.cy = y;
    }

    fn first_nonblank(&mut self) {
        self.cx = self.lines[self.cy].iter().position(|c| !c.is_whitespace()).unwrap_or(0);
    }

    fn search_from(&self, pat: &str, forward: bool) -> Option<(usize, usize)> {
        let pat: Vec<char> = pat.chars().collect();
        if pat.is_empty() {
            return None;
        }
        let n = self.lines.len();
        let find_in = |line: &Vec<char>, from: usize, to: usize| -> Option<usize> {
            if to > line.len() || from > to {
                return None;
            }
            let mut i = from;
            while i + pat.len() <= to.min(line.len()) {
                if line[i..i + pat.len()] == pat[..] {
                    return Some(i);
                }
                i += 1;
            }
            None
        };
        let rfind_in = |line: &Vec<char>, before: usize| -> Option<usize> {
            let mut i = before.min(line.len());
            while i > 0 {
                i -= 1;
                if i + pat.len() <= line.len() && line[i..i + pat.len()] == pat[..] {
                    return Some(i);
                }
            }
            None
        };
        if forward {
            if let Some(x) = find_in(&self.lines[self.cy], self.cx + 1, self.lines[self.cy].len()) {
                return Some((x, self.cy));
            }
            for k in 1..=n {
                let y = (self.cy + k) % n;
                if let Some(x) = find_in(&self.lines[y], 0, self.lines[y].len()) {
                    return Some((x, y));
                }
            }
        } else {
            if let Some(x) = rfind_in(&self.lines[self.cy], self.cx) {
                return Some((x, self.cy));
            }
            for k in 1..=n {
                let y = (self.cy + n - k) % n;
                if let Some(x) = rfind_in(&self.lines[y], self.lines[y].len()) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    fn do_search(&mut self, forward: bool) {
        let Some(pat) = self.last_search.clone() else {
            self.message = "E35: No previous regular expression".into();
            return;
        };
        match self.search_from(&pat, forward) {
            Some((x, y)) => {
                let wrapped = if forward {
                    y < self.cy || (y == self.cy && x <= self.cx)
                } else {
                    y > self.cy || (y == self.cy && x >= self.cx)
                };
                self.cx = x;
                self.cy = y;
                if wrapped {
                    self.message = if forward {
                        "search hit BOTTOM, continuing at TOP".into()
                    } else {
                        "search hit TOP, continuing at BOTTOM".into()
                    };
                }
            }
            None => self.message = format!("E486: Pattern not found: {pat}"),
        }
    }

    // ---- edits -------------------------------------------------------------

    fn delete_lines(&mut self, n: usize) {
        self.snapshot();
        let end = (self.cy + n).min(self.lines.len());
        self.yank = self.lines.drain(self.cy..end).collect();
        self.yank_linewise = true;
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        self.clamp();
        self.first_nonblank();
    }

    fn yank_lines(&mut self, n: usize) {
        let end = (self.cy + n).min(self.lines.len());
        self.yank = self.lines[self.cy..end].to_vec();
        self.yank_linewise = true;
        self.message = format!(
            "{} line{} yanked",
            end - self.cy,
            if end - self.cy == 1 { "" } else { "s" }
        );
    }

    fn put(&mut self, after: bool) {
        if self.yank.is_empty() {
            return;
        }
        self.snapshot();
        if self.yank_linewise {
            let at = if after { self.cy + 1 } else { self.cy };
            for (i, l) in self.yank.clone().into_iter().enumerate() {
                self.lines.insert(at + i, l);
            }
            self.cy = at;
            self.first_nonblank();
        } else {
            let chars: Vec<char> = self.yank[0].clone();
            let at = if after && !self.lines[self.cy].is_empty() {
                self.cx + 1
            } else {
                self.cx
            };
            let line = &mut self.lines[self.cy];
            for (i, c) in chars.iter().enumerate() {
                line.insert(at + i, *c);
            }
            self.cx = at + chars.len().saturating_sub(1);
        }
    }

    fn delete_char(&mut self, n: usize) {
        if self.line_len() == 0 {
            return;
        }
        self.snapshot();
        let end = (self.cx + n).min(self.line_len());
        let removed: Vec<char> = self.lines[self.cy].drain(self.cx..end).collect();
        self.yank = vec![removed];
        self.yank_linewise = false;
        self.clamp();
    }

    fn delete_word(&mut self, n: usize) {
        self.snapshot();
        let (sx, sy) = (self.cx, self.cy);
        for _ in 0..n {
            self.next_word();
        }
        if self.cy != sy {
            // only delete to end of the starting line (vi's dw stops there)
            self.cy = sy;
            self.cx = self.lines[sy].len();
        }
        let end = self.cx;
        let removed: Vec<char> = self.lines[sy].drain(sx..end).collect();
        self.yank = vec![removed];
        self.yank_linewise = false;
        self.cx = sx;
        self.clamp();
    }

    fn join(&mut self) {
        if self.cy + 1 >= self.lines.len() {
            return;
        }
        self.snapshot();
        let next = self.lines.remove(self.cy + 1);
        let line = &mut self.lines[self.cy];
        let join_at = line.len();
        if !line.is_empty() && !next.is_empty() {
            line.push(' ');
        }
        let trimmed: Vec<char> = next.into_iter().skip_while(|c| c.is_whitespace()).collect();
        line.extend(trimmed);
        self.cx = join_at;
        self.clamp();
    }

    fn enter_insert(&mut self) {
        self.insert_snapshot = Some((self.lines.clone(), self.cx, self.cy));
        self.mode = Mode::Insert;
        self.message = "-- INSERT --".into();
    }

    fn leave_insert(&mut self) {
        if let Some(snap) = self.insert_snapshot.take() {
            if snap.0 != self.lines {
                self.undo.push(snap);
                if self.undo.len() > UNDO_MAX {
                    self.undo.remove(0);
                }
                self.dirty = true;
            }
        }
        self.mode = Mode::Normal;
        self.message.clear();
        self.cx = self.cx.saturating_sub(1);
        self.clamp();
    }

    fn undo(&mut self) {
        match self.undo.pop() {
            Some((lines, cx, cy)) => {
                self.lines = lines;
                self.cx = cx;
                self.cy = cy;
                self.clamp();
                self.message = "undone".into();
            }
            None => self.message = "Already at oldest change".into(),
        }
    }

    // ---- the key handler ---------------------------------------------------

    pub fn key(&mut self, k: Key) -> Event {
        self.keys_seen += 1;
        let ev = match self.mode {
            Mode::Normal => self.normal_key(k),
            Mode::Insert => {
                self.insert_key(k);
                Event::None
            }
            Mode::Command | Mode::Search => self.line_key(k),
        };
        self.clamp();
        ev
    }

    fn insert_key(&mut self, k: Key) {
        match k {
            Key::Escape | Key::Ctrl('c') | Key::Ctrl('[') => self.leave_insert(),
            Key::Char(c) => {
                let cx = self.cx;
                self.lines[self.cy].insert(cx, c);
                self.cx += 1;
            }
            Key::Enter => {
                let rest: Vec<char> = self.lines[self.cy].split_off(self.cx);
                self.lines.insert(self.cy + 1, rest);
                self.cy += 1;
                self.cx = 0;
            }
            Key::Backspace => {
                if self.cx > 0 {
                    self.cx -= 1;
                    let cx = self.cx;
                    self.lines[self.cy].remove(cx);
                } else if self.cy > 0 {
                    let line = self.lines.remove(self.cy);
                    self.cy -= 1;
                    self.cx = self.lines[self.cy].len();
                    self.lines[self.cy].extend(line);
                }
            }
            Key::Tab => {
                for _ in 0..4 {
                    let cx = self.cx;
                    self.lines[self.cy].insert(cx, ' ');
                    self.cx += 1;
                }
            }
            Key::Left => self.cx = self.cx.saturating_sub(1),
            Key::Right => self.cx = (self.cx + 1).min(self.line_len()),
            Key::Up => self.cy = self.cy.saturating_sub(1),
            Key::Down => self.cy = (self.cy + 1).min(self.lines.len() - 1),
            Key::Home => self.cx = 0,
            Key::End => self.cx = self.line_len(),
            _ => {}
        }
    }

    fn line_key(&mut self, k: Key) -> Event {
        match k {
            Key::Escape | Key::Ctrl('c') => {
                self.mode = Mode::Normal;
                self.cmdline.clear();
                Event::None
            }
            Key::Backspace => {
                if self.cmdline.pop().is_none() {
                    self.mode = Mode::Normal;
                }
                Event::None
            }
            Key::Char(c) => {
                self.cmdline.push(c);
                Event::None
            }
            Key::Enter => {
                let line = std::mem::take(&mut self.cmdline);
                let was = self.mode;
                self.mode = Mode::Normal;
                if was == Mode::Search {
                    if !line.is_empty() {
                        self.last_search = Some(line);
                    }
                    self.do_search(true);
                    Event::None
                } else {
                    self.ex_command(line.trim())
                }
            }
            _ => Event::None,
        }
    }

    fn ex_command(&mut self, cmd: &str) -> Event {
        match cmd {
            "" => Event::None,
            "q" => Event::Quit { force: false },
            "q!" => Event::Quit { force: true },
            "w" => Event::Write,
            "wq" | "x" | "wq!" => Event::WriteQuit,
            n if n.chars().all(|c| c.is_ascii_digit()) => {
                let target: usize = n.parse().unwrap_or(1);
                self.cy = target.max(1).min(self.lines.len()) - 1;
                self.first_nonblank();
                Event::None
            }
            other => Event::Unknown(other.to_string()),
        }
    }

    fn normal_key(&mut self, k: Key) -> Event {
        // counts
        if let Key::Char(d) = k {
            if d.is_ascii_digit() && (d != '0' || self.count.is_some()) && self.pending.is_empty() {
                let v = self.count.unwrap_or(0) * 10 + d.to_digit(10).unwrap() as usize;
                self.count = Some(v.min(9999));
                return Event::None;
            }
        }
        // second key of a two-key command
        if !self.pending.is_empty() {
            let first = self.pending.clone();
            self.pending.clear();
            let n = self.take_count();
            match (first.as_str(), k) {
                ("d", Key::Char('d')) => self.delete_lines(n),
                ("d", Key::Char('w')) => self.delete_word(n),
                ("d", Key::Char('$')) => {
                    self.snapshot();
                    let cx = self.cx;
                    let removed: Vec<char> = self.lines[self.cy].drain(cx..).collect();
                    self.yank = vec![removed];
                    self.yank_linewise = false;
                }
                ("y", Key::Char('y')) => self.yank_lines(n),
                ("y", Key::Char('w')) => {
                    let (sx, sy) = (self.cx, self.cy);
                    self.next_word();
                    let end = if self.cy == sy { self.cx } else { self.lines[sy].len() };
                    self.yank = vec![self.lines[sy][sx..end].to_vec()];
                    self.yank_linewise = false;
                    self.cx = sx;
                    self.cy = sy;
                }
                ("g", Key::Char('g')) => {
                    self.cy = if self.count.is_some() {
                        n.max(1).min(self.lines.len()) - 1
                    } else {
                        0
                    };
                    self.first_nonblank();
                }
                ("r", Key::Char(c)) => {
                    if self.line_len() > 0 {
                        self.snapshot();
                        let cx = self.cx;
                        self.lines[self.cy][cx] = c;
                    }
                }
                ("Z", Key::Char('Z')) => return Event::WriteQuit,
                ("Z", Key::Char('Q')) => return Event::Quit { force: true },
                _ => {}
            }
            return Event::None;
        }
        let n = self.take_count();
        match k {
            Key::Char('h') | Key::Left | Key::Backspace => self.cx = self.cx.saturating_sub(n),
            Key::Char('l') | Key::Right | Key::Char(' ') => self.cx += n,
            Key::Char('j') | Key::Down | Key::Ctrl('n') => self.cy = (self.cy + n).min(self.lines.len() - 1),
            Key::Char('k') | Key::Up | Key::Ctrl('p') => self.cy = self.cy.saturating_sub(n),
            Key::Char('w') => {
                for _ in 0..n {
                    self.next_word();
                }
            }
            Key::Char('b') => {
                for _ in 0..n {
                    self.prev_word();
                }
            }
            Key::Char('e') => {
                for _ in 0..n {
                    self.word_end();
                }
            }
            Key::Char('0') | Key::Home => self.cx = 0,
            Key::Char('$') | Key::End => self.cx = self.line_len().saturating_sub(1),
            Key::Char('^') => self.first_nonblank(),
            Key::Char('G') => {
                self.cy = if self.count.is_some() {
                    n.max(1).min(self.lines.len()) - 1
                } else {
                    self.lines.len() - 1
                };
                self.first_nonblank();
            }
            Key::Enter | Key::Char('+') => {
                self.cy = (self.cy + n).min(self.lines.len() - 1);
                self.first_nonblank();
            }
            Key::Char('-') => {
                self.cy = self.cy.saturating_sub(n);
                self.first_nonblank();
            }
            Key::PageDown | Key::Ctrl('f') => self.cy = (self.cy + 20).min(self.lines.len() - 1),
            Key::PageUp | Key::Ctrl('b') => self.cy = self.cy.saturating_sub(20),
            Key::Char('x') | Key::Delete => self.delete_char(n),
            Key::Char('X') => {
                if self.cx > 0 {
                    self.cx -= 1;
                    self.delete_char(1);
                }
            }
            Key::Char('D') => {
                self.snapshot();
                let cx = self.cx;
                let removed: Vec<char> = self.lines[self.cy].drain(cx..).collect();
                self.yank = vec![removed];
                self.yank_linewise = false;
            }
            Key::Char('d') | Key::Char('y') | Key::Char('g') | Key::Char('r') | Key::Char('Z') => {
                self.pending = k.as_char().unwrap().to_string();
                // keep the count for the second key
                self.count = Some(n);
                if n == 1 {
                    self.count = None;
                }
            }
            Key::Char('p') => self.put(true),
            Key::Char('P') => self.put(false),
            Key::Char('J') => self.join(),
            Key::Char('u') => self.undo(),
            Key::Char('~') => {
                if let Some(c) = self.current() {
                    self.snapshot();
                    let cx = self.cx;
                    self.lines[self.cy][cx] = if c.is_uppercase() {
                        c.to_lowercase().next().unwrap_or(c)
                    } else {
                        c.to_uppercase().next().unwrap_or(c)
                    };
                    self.cx += 1;
                }
            }
            Key::Char('i') => self.enter_insert(),
            Key::Char('a') => {
                self.enter_insert();
                self.cx = (self.cx + 1).min(self.line_len());
            }
            Key::Char('I') => {
                self.first_nonblank();
                self.enter_insert();
            }
            Key::Char('A') => {
                self.enter_insert();
                self.cx = self.line_len();
            }
            Key::Char('o') => {
                self.enter_insert();
                self.lines.insert(self.cy + 1, Vec::new());
                self.cy += 1;
                self.cx = 0;
            }
            Key::Char('O') => {
                self.enter_insert();
                self.lines.insert(self.cy, Vec::new());
                self.cx = 0;
            }
            Key::Char(':') => {
                self.mode = Mode::Command;
                self.cmdline.clear();
                self.message.clear();
            }
            Key::Char('/') => {
                self.mode = Mode::Search;
                self.cmdline.clear();
                self.message.clear();
            }
            Key::Char('n') => self.do_search(true),
            Key::Char('N') => self.do_search(false),
            Key::Escape => {
                self.count = None;
                self.message.clear();
            }
            _ => {}
        }
        Event::None
    }

    /// The bottom line as vi shows it.
    pub fn status_line(&self) -> String {
        match self.mode {
            Mode::Command => format!(":{}", self.cmdline),
            Mode::Search => format!("/{}", self.cmdline),
            _ => self.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &mut Vi, s: &str) -> Event {
        let mut ev = Event::None;
        for c in s.chars() {
            ev = match c {
                '\u{1b}' => v.key(Key::Escape),
                '\n' => v.key(Key::Enter),
                c => v.key(Key::Char(c)),
            };
        }
        ev
    }

    #[test]
    fn motions() {
        let mut v = Vi::new("hello big world\nsecond line\n\nfourth");
        keys(&mut v, "lll");
        assert_eq!((v.cx, v.cy), (3, 0));
        keys(&mut v, "w");
        assert_eq!((v.cx, v.cy), (6, 0));
        keys(&mut v, "w");
        assert_eq!((v.cx, v.cy), (10, 0));
        keys(&mut v, "b");
        assert_eq!((v.cx, v.cy), (6, 0));
        keys(&mut v, "$");
        assert_eq!((v.cx, v.cy), (14, 0));
        keys(&mut v, "j0");
        assert_eq!((v.cx, v.cy), (0, 1));
        keys(&mut v, "G");
        assert_eq!(v.cy, 3);
        keys(&mut v, "gg");
        assert_eq!(v.cy, 0);
        keys(&mut v, "3l");
        assert_eq!(v.cx, 3);
        keys(&mut v, "e");
        assert_eq!(v.cx, 4);
        keys(&mut v, "ww");
        assert_eq!(v.cy, 0);
        keys(&mut v, "w");
        assert_eq!((v.cx, v.cy), (0, 1));
    }

    #[test]
    fn edits_and_undo() {
        let mut v = Vi::new("abc def\nline two\nline three");
        keys(&mut v, "x");
        assert_eq!(v.text(), "bc def\nline two\nline three\n");
        keys(&mut v, "dw");
        assert_eq!(v.text(), "def\nline two\nline three\n");
        keys(&mut v, "dd");
        assert_eq!(v.text(), "line two\nline three\n");
        keys(&mut v, "yyp");
        assert_eq!(v.text(), "line two\nline two\nline three\n");
        keys(&mut v, "u");
        assert_eq!(v.text(), "line two\nline three\n");
        keys(&mut v, "u");
        assert_eq!(v.text(), "def\nline two\nline three\n");
        keys(&mut v, "2dd");
        assert_eq!(v.text(), "line three\n");
        keys(&mut v, "rL");
        assert_eq!(v.text(), "Line three\n");
        keys(&mut v, "J");
        assert!(v.dirty);
    }

    #[test]
    fn insert_mode() {
        let mut v = Vi::new("world");
        keys(&mut v, "ihello \u{1b}");
        assert_eq!(v.text(), "hello world\n");
        assert_eq!(v.mode, Mode::Normal);
        keys(&mut v, "A!\u{1b}");
        assert_eq!(v.text(), "hello world!\n");
        keys(&mut v, "onext\u{1b}");
        assert_eq!(v.text(), "hello world!\nnext\n");
        keys(&mut v, "u");
        assert_eq!(v.text(), "hello world!\n");
    }

    #[test]
    fn ex_commands_and_search() {
        let mut v = Vi::new("one\ntwo key\nthree\nkey four");
        assert_eq!(keys(&mut v, ":q\n"), Event::Quit { force: false });
        assert_eq!(keys(&mut v, ":q!\n"), Event::Quit { force: true });
        assert_eq!(keys(&mut v, ":wq\n"), Event::WriteQuit);
        assert_eq!(keys(&mut v, "ZZ"), Event::WriteQuit);
        assert_eq!(keys(&mut v, ":frobnicate\n"), Event::Unknown("frobnicate".into()));
        keys(&mut v, "/key\n");
        assert_eq!((v.cx, v.cy), (4, 1));
        keys(&mut v, "n");
        assert_eq!((v.cx, v.cy), (0, 3));
        keys(&mut v, "n");
        assert_eq!((v.cx, v.cy), (4, 1));
        assert!(v.message.contains("BOTTOM"));
        keys(&mut v, ":3\n");
        assert_eq!(v.cy, 2);
        keys(&mut v, "/zzz\n");
        assert!(v.message.contains("Pattern not found"));
    }
}
