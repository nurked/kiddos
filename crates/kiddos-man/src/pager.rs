//! `less`-style paging for long output on a tty.

use kiddos_kernel::{Console, Interrupted, Key, Proc};

/// Show `text`. On a tty longer than the screen, page it: Space/PageDown
/// next page, Enter/Down one line, q/Escape quit. Off-tty, just print.
pub fn page(p: &Proc, text: &str) -> Result<(), Interrupted> {
    let lines: Vec<&str> = text.lines().collect();
    let (_, rows) = p.size();
    let page_rows = (rows as usize).saturating_sub(1).max(1);
    if !p.stdout_is_tty() || lines.len() <= page_rows {
        p.print(text);
        if !text.ends_with('\n') && !text.is_empty() {
            p.print("\n");
        }
        return Ok(());
    }
    let mut top = 0usize;
    let mut shown = 0usize; // lines printed so far
    let prompt = p.t("man-press-key", &[]);
    loop {
        let end = (top + page_rows).min(lines.len());
        for l in &lines[shown.max(top)..end] {
            p.print(l);
            p.print("\n");
        }
        shown = end;
        if shown >= lines.len() {
            return Ok(());
        }
        p.print(&format!("\x1b[7m{prompt}\x1b[0m"));
        let key = loop {
            match p.readkey()? {
                Key::Char(' ') | Key::PageDown | Key::Char('f') => break Some(page_rows),
                Key::Enter | Key::Down | Key::Char('j') => break Some(1),
                Key::Char('q') | Key::Escape | Key::Ctrl('c') => break None,
                _ => {}
            }
        };
        // erase the prompt line
        p.print("\r\x1b[K");
        match key {
            Some(n) => top = (top + n).min(lines.len().saturating_sub(page_rows)),
            None => return Ok(()),
        }
    }
}
