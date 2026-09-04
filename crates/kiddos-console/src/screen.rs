//! The cell buffer. Retained-mode: the renderer polls `generation()` and only
//! redraws when it changed.

use crate::color::colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: u8,
    pub bg: u8,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        ch: ' ',
        fg: colors::DEFAULT_FG,
        bg: colors::DEFAULT_BG,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ansi {
    Ground,
    Esc,
    Csi,
}

#[derive(Debug, Clone)]
pub struct Screen {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    cx: u16,
    cy: u16,
    cursor_visible: bool,
    fg: u8,
    bg: u8,
    bold: bool,
    generation: u64,
    bells: u32,
    ansi: Ansi,
    csi_buf: String,
    /// Set by programs that draw the whole screen themselves; the shell uses
    /// it to know whether to reset colors when a program exits.
    pub title: String,
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Screen {
        assert!(cols >= 1 && rows >= 1, "screen too small");
        Screen {
            cols,
            rows,
            cells: vec![Cell::BLANK; cols as usize * rows as usize],
            cx: 0,
            cy: 0,
            cursor_visible: true,
            fg: colors::DEFAULT_FG,
            bg: colors::DEFAULT_BG,
            bold: false,
            generation: 1,
            bells: 0,
            ansi: Ansi::Ground,
            csi_buf: String::new(),
            title: String::new(),
        }
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }
    pub fn rows(&self) -> u16 {
        self.rows
    }
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }
    /// Bumps every time anything visible changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
    pub fn cell(&self, x: u16, y: u16) -> Cell {
        if x < self.cols && y < self.rows {
            self.cells[y as usize * self.cols as usize + x as usize]
        } else {
            Cell::BLANK
        }
    }
    pub fn cursor(&self) -> (u16, u16) {
        (self.cx, self.cy)
    }
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }
    pub fn colors(&self) -> (u8, u8) {
        (self.fg, self.bg)
    }
    /// Number of BEL characters written since the last call (renderer beeps).
    pub fn take_bells(&mut self) -> u32 {
        std::mem::take(&mut self.bells)
    }

    fn touch(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_cursor(&mut self, x: u16, y: u16) {
        self.cx = x.min(self.cols - 1);
        self.cy = y.min(self.rows - 1);
        self.touch();
    }
    pub fn show_cursor(&mut self, v: bool) {
        if self.cursor_visible != v {
            self.cursor_visible = v;
            self.touch();
        }
    }
    pub fn set_colors(&mut self, fg: u8, bg: u8) {
        self.fg = fg & 15;
        self.bg = bg & 15;
        self.bold = false;
    }
    pub fn reset_colors(&mut self) {
        self.set_colors(colors::DEFAULT_FG, colors::DEFAULT_BG);
    }

    pub fn put(&mut self, x: u16, y: u16, ch: char, fg: u8, bg: u8) {
        if x < self.cols && y < self.rows {
            let i = y as usize * self.cols as usize + x as usize;
            let c = Cell {
                ch,
                fg: fg & 15,
                bg: bg & 15,
            };
            if self.cells[i] != c {
                self.cells[i] = c;
                self.touch();
            }
        }
    }

    pub fn clear(&mut self, bg: u8) {
        let bg = bg & 15;
        for c in &mut self.cells {
            *c = Cell {
                ch: ' ',
                fg: self.fg,
                bg,
            };
        }
        self.bg = bg;
        self.cx = 0;
        self.cy = 0;
        self.touch();
    }

    pub fn scroll_up(&mut self, n: u16) {
        let n = n.min(self.rows) as usize;
        if n == 0 {
            return;
        }
        let w = self.cols as usize;
        self.cells.drain(0..n * w);
        let blank = Cell {
            ch: ' ',
            fg: self.fg,
            bg: self.bg,
        };
        self.cells.extend(std::iter::repeat_n(blank, n * w));
        self.touch();
    }

    fn newline(&mut self) {
        self.cx = 0;
        if self.cy + 1 >= self.rows {
            self.scroll_up(1);
        } else {
            self.cy += 1;
        }
    }

    fn put_at_cursor(&mut self, ch: char) {
        if self.cx >= self.cols {
            self.newline();
        }
        let fg = if self.bold && self.fg < 8 { self.fg + 8 } else { self.fg };
        let (x, y) = (self.cx, self.cy);
        self.put(x, y, ch, fg, self.bg);
        self.cx += 1;
        self.touch();
    }

    /// Write text at the cursor. Understands `\n`, `\r`, `\t`, backspace,
    /// BEL, and the ANSI subset: SGR colors, `ESC[2J`, `ESC[H`, `ESC[K`,
    /// `ESC[row;colH`, `ESC[?25l/h` (cursor hide/show).
    pub fn write_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.write_char(ch);
        }
    }

    pub fn write_char(&mut self, ch: char) {
        match self.ansi {
            Ansi::Ground => match ch {
                '\x1b' => self.ansi = Ansi::Esc,
                '\n' => self.newline(),
                '\r' => {
                    self.cx = 0;
                    self.touch();
                }
                '\t' => {
                    let next = (self.cx / 8 + 1) * 8;
                    while self.cx < next && self.cx < self.cols {
                        self.put_at_cursor(' ');
                    }
                }
                '\x08' => {
                    if self.cx > 0 {
                        self.cx -= 1;
                        self.touch();
                    }
                }
                '\x07' => self.bells += 1,
                c if (c as u32) < 0x20 => {}
                c => self.put_at_cursor(c),
            },
            Ansi::Esc => {
                if ch == '[' {
                    self.ansi = Ansi::Csi;
                    self.csi_buf.clear();
                } else {
                    self.ansi = Ansi::Ground;
                }
            }
            Ansi::Csi => {
                if ch.is_ascii_digit() || ch == ';' || ch == '?' {
                    if self.csi_buf.len() < 32 {
                        self.csi_buf.push(ch);
                    }
                } else {
                    let buf = std::mem::take(&mut self.csi_buf);
                    self.ansi = Ansi::Ground;
                    self.csi(&buf, ch);
                }
            }
        }
    }

    fn csi(&mut self, params: &str, cmd: char) {
        let private = params.starts_with('?');
        let nums: Vec<u16> = params
            .trim_start_matches('?')
            .split(';')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        match cmd {
            'm' => {
                if nums.is_empty() {
                    self.reset_colors();
                }
                for n in nums {
                    match n {
                        0 => self.reset_colors(),
                        1 => self.bold = true,
                        22 => self.bold = false,
                        30..=37 => self.fg = ansi_to_cga((n - 30) as u8),
                        90..=97 => self.fg = ansi_to_cga((n - 90) as u8) + 8,
                        39 => self.fg = colors::DEFAULT_FG,
                        40..=47 => self.bg = ansi_to_cga((n - 40) as u8),
                        100..=107 => self.bg = ansi_to_cga((n - 100) as u8) + 8,
                        49 => self.bg = colors::DEFAULT_BG,
                        _ => {}
                    }
                }
            }
            'J' => {
                let bg = self.bg;
                self.clear(bg);
            }
            'H' | 'f' => {
                let row = nums.first().copied().unwrap_or(1).max(1) - 1;
                let col = nums.get(1).copied().unwrap_or(1).max(1) - 1;
                self.set_cursor(col, row);
            }
            'K' => {
                let (y, bg, fg) = (self.cy, self.bg, self.fg);
                for x in self.cx..self.cols {
                    self.put(x, y, ' ', fg, bg);
                }
            }
            'A' => {
                let n = nums.first().copied().unwrap_or(1);
                self.set_cursor(self.cx, self.cy.saturating_sub(n));
            }
            'B' => {
                let n = nums.first().copied().unwrap_or(1);
                self.set_cursor(self.cx, self.cy + n);
            }
            'C' => {
                let n = nums.first().copied().unwrap_or(1);
                self.set_cursor(self.cx + n, self.cy);
            }
            'D' => {
                let n = nums.first().copied().unwrap_or(1);
                self.set_cursor(self.cx.saturating_sub(n), self.cy);
            }
            'l' if private && nums.first() == Some(&25) => self.show_cursor(false),
            'h' if private && nums.first() == Some(&25) => self.show_cursor(true),
            _ => {}
        }
    }

    /// Copy another screen's contents over this one (used to restore the
    /// screen after a full-screen program). The generation still advances.
    pub fn restore_from(&mut self, other: &Screen) {
        let generation = self.generation;
        *self = other.clone();
        self.generation = generation.max(other.generation).wrapping_add(1);
    }

    /// One row as text (trailing spaces trimmed).
    pub fn line(&self, y: u16) -> String {
        let w = self.cols as usize;
        let row = &self.cells[y as usize * w..(y as usize + 1) * w];
        let s: String = row.iter().map(|c| c.ch).collect();
        s.trim_end().to_string()
    }

    /// The whole screen as text, rows joined with `\n`, trailing blank rows
    /// removed. This is what headless tests diff against.
    pub fn text(&self) -> String {
        let mut lines: Vec<String> = (0..self.rows).map(|y| self.line(y)).collect();
        while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        lines.join("\n")
    }
}

/// ANSI color order is BGR-ish (1=red, 4=blue); CGA is RGB-ish (1=blue,
/// 4=red). Swap bits 0 and 2.
fn ansi_to_cga(n: u8) -> u8 {
    (n & 2) | ((n & 1) << 2) | ((n & 4) >> 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_and_wraps() {
        let mut s = Screen::new(20, 5);
        s.write_str("hello\nworld");
        assert_eq!(s.line(0), "hello");
        assert_eq!(s.line(1), "world");
        assert_eq!(s.cursor(), (5, 1));
        s.write_str("\n");
        s.write_str("12345678901234567890abc");
        assert_eq!(s.line(2), "12345678901234567890");
        assert_eq!(s.line(3), "abc");
    }

    #[test]
    fn scrolls() {
        let mut s = Screen::new(20, 3);
        s.write_str("a\nb\nc\nd");
        assert_eq!(s.text(), "b\nc\nd");
        assert_eq!(s.cursor(), (1, 2));
    }

    #[test]
    fn ansi_colors() {
        let mut s = Screen::new(20, 3);
        s.write_str("\x1b[31mR\x1b[0m\x1b[1;34mB\x1b[m");
        assert_eq!(s.cell(0, 0).fg, colors::RED);
        assert_eq!(s.cell(1, 0).fg, colors::LIGHT_BLUE);
        assert_eq!(s.colors(), (colors::DEFAULT_FG, colors::DEFAULT_BG));
    }

    #[test]
    fn ansi_cursor_and_clear() {
        let mut s = Screen::new(20, 5);
        s.write_str("junk\x1b[2Jx\x1b[3;4Hy");
        assert_eq!(s.line(0), "x");
        assert_eq!(s.cell(3, 2).ch, 'y');
        s.write_str("\x1b[?25l");
        assert!(!s.cursor_visible());
    }

    #[test]
    fn tabs_and_backspace() {
        let mut s = Screen::new(20, 3);
        s.write_str("ab\tc");
        assert_eq!(s.line(0), "ab      c");
        s.write_str("\x08 ");
        assert_eq!(s.line(0), "ab");
    }
}
