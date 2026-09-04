//! EndBASIC's `Console` on top of a KidDOS process.

use async_channel::Sender;
use async_trait::async_trait;
use endbasic_core::exec::Signal;
use endbasic_std::console::{CharsXY, ClearType, Console as EbConsole, Key as EbKey};
use kiddos_console::{colors, Key, Screen};
use kiddos_kernel::{Console, Proc};
use std::io;
use std::sync::Arc;

pub struct KidConsole {
    p: Arc<Proc>,
    signals: Sender<Signal>,
    saved: Option<Screen>,
    fg: Option<u8>,
    bg: Option<u8>,
}

impl KidConsole {
    pub fn new(p: Arc<Proc>, signals: Sender<Signal>) -> KidConsole {
        KidConsole {
            p,
            signals,
            saved: None,
            fg: None,
            bg: None,
        }
    }

    fn map_key(&self, k: Key) -> EbKey {
        match k {
            Key::Char(c) => EbKey::Char(c),
            Key::Enter => EbKey::CarriageReturn,
            Key::Backspace => EbKey::Backspace,
            Key::Tab | Key::BackTab => EbKey::Tab,
            Key::Escape => EbKey::Escape,
            Key::Up => EbKey::ArrowUp,
            Key::Down => EbKey::ArrowDown,
            Key::Left => EbKey::ArrowLeft,
            Key::Right => EbKey::ArrowRight,
            Key::Home | Key::Ctrl('a') => EbKey::Home,
            Key::End | Key::Ctrl('e') => EbKey::End,
            Key::PageUp => EbKey::PageUp,
            Key::PageDown => EbKey::PageDown,
            Key::Ctrl('c') => {
                let _ = self.signals.try_send(Signal::Break);
                EbKey::Interrupt
            }
            Key::Ctrl('d') => EbKey::Eof,
            _ => EbKey::Unknown,
        }
    }
}

/// BASIC colors are the classic CGA/QBasic numbers: 1 blue, 4 red, 14 yellow.
fn to_cga(c: Option<u8>, default: u8) -> u8 {
    match c {
        None => default,
        Some(n) => n % 16,
    }
}

#[async_trait(?Send)]
impl EbConsole for KidConsole {
    fn clear(&mut self, how: ClearType) -> io::Result<()> {
        let (cols, _) = self.p.size();
        let (x, y) = self.p.cursor_pos();
        let (fg, bg) = (to_cga(self.fg, colors::DEFAULT_FG), to_cga(self.bg, colors::DEFAULT_BG));
        match how {
            ClearType::All => self.p.clear(bg),
            ClearType::CurrentLine => {
                for cx in 0..cols {
                    self.p.put(cx, y, ' ', fg, bg);
                }
            }
            ClearType::PreviousChar => {
                if x > 0 {
                    self.p.put(x - 1, y, ' ', fg, bg);
                    self.p.cursor(x - 1, y);
                }
            }
            ClearType::UntilNewLine => {
                for cx in x..cols {
                    self.p.put(cx, y, ' ', fg, bg);
                }
            }
        }
        Ok(())
    }

    fn color(&self) -> (Option<u8>, Option<u8>) {
        (self.fg, self.bg)
    }

    fn set_color(&mut self, fg: Option<u8>, bg: Option<u8>) -> io::Result<()> {
        self.fg = fg;
        self.bg = bg;
        self.p
            .set_color(to_cga(fg, colors::DEFAULT_FG), to_cga(bg, colors::DEFAULT_BG));
        Ok(())
    }

    fn enter_alt(&mut self) -> io::Result<()> {
        if self.saved.is_none() {
            self.saved = Some(self.p.kernel().screen.lock().clone());
            self.p.clear(to_cga(self.bg, colors::DEFAULT_BG));
        }
        Ok(())
    }

    fn leave_alt(&mut self) -> io::Result<()> {
        if let Some(saved) = self.saved.take() {
            self.p.kernel().screen.lock().restore_from(&saved);
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.p.cursor_show(false);
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.p.cursor_show(true);
        Ok(())
    }

    fn is_interactive(&self) -> bool {
        self.p.stdin_is_tty()
    }

    fn locate(&mut self, pos: CharsXY) -> io::Result<()> {
        self.p.cursor(pos.x, pos.y);
        Ok(())
    }

    fn move_within_line(&mut self, off: i16) -> io::Result<()> {
        let (x, y) = self.p.cursor_pos();
        let nx = (x as i32 + off as i32).max(0) as u16;
        self.p.cursor(nx, y);
        Ok(())
    }

    fn print(&mut self, text: &str) -> io::Result<()> {
        let text = endbasic_std::console::remove_control_chars(text.to_owned());
        self.p.print(&text);
        self.p.print("\n");
        Ok(())
    }

    fn write(&mut self, text: &str) -> io::Result<()> {
        let text = endbasic_std::console::remove_control_chars(text.to_owned());
        self.p.print(&text);
        Ok(())
    }

    async fn poll_key(&mut self) -> io::Result<Option<EbKey>> {
        if self.p.killed() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }
        Ok(self.p.getkey().map(|k| self.map_key(k)))
    }

    async fn read_key(&mut self) -> io::Result<EbKey> {
        match self.p.readkey() {
            Ok(k) => Ok(self.map_key(k)),
            Err(_) => Err(io::Error::new(io::ErrorKind::Interrupted, "stopped")),
        }
    }

    fn size_chars(&self) -> io::Result<CharsXY> {
        let (cols, rows) = self.p.size();
        Ok(CharsXY { x: cols, y: rows })
    }

    fn sync_now(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn set_sync(&mut self, _enabled: bool) -> io::Result<bool> {
        Ok(true)
    }
}
