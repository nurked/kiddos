//! Keyboard vocabulary. The host maps physical events onto this; programs
//! never see scancodes.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable character (already shifted / localized by the host).
    Char(char),
    Enter,
    Backspace,
    Tab,
    BackTab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// F1..F12
    F(u8),
    /// Ctrl + a lowercase ASCII letter, e.g. `Ctrl('c')`.
    Ctrl(char),
    /// Alt + a lowercase ASCII letter.
    Alt(char),
}

/// A key going down or up. Auto-repeat does not produce events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: Key,
    pub down: bool,
}

impl Key {
    /// For `Char` keys, the same letter in the other case: a key released
    /// after Shift went up must still clear the held state.
    pub fn case_swapped(&self) -> Option<Key> {
        match self {
            Key::Char(c) if c.is_lowercase() => c.to_uppercase().next().map(Key::Char),
            Key::Char(c) if c.is_uppercase() => c.to_lowercase().next().map(Key::Char),
            _ => None,
        }
    }

    /// Parse a key name as used by headless scripts and docs:
    /// `enter`, `tab`, `up`, `ctrl-c`, `f1`, `esc`, `space`.
    pub fn parse_name(name: &str) -> Option<Key> {
        let n = name.trim().to_ascii_lowercase();
        Some(match n.as_str() {
            "enter" | "return" | "cr" => Key::Enter,
            "backspace" | "bs" => Key::Backspace,
            "tab" => Key::Tab,
            "backtab" | "shift-tab" => Key::BackTab,
            "esc" | "escape" => Key::Escape,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "home" => Key::Home,
            "end" => Key::End,
            "pgup" | "pageup" => Key::PageUp,
            "pgdn" | "pagedown" => Key::PageDown,
            "ins" | "insert" => Key::Insert,
            "del" | "delete" => Key::Delete,
            "space" => Key::Char(' '),
            _ => {
                if let Some(rest) = n.strip_prefix("ctrl-") {
                    let c = rest.chars().next()?;
                    if rest.chars().count() == 1 && c.is_ascii_alphabetic() {
                        return Some(Key::Ctrl(c));
                    }
                    return None;
                }
                if let Some(rest) = n.strip_prefix("alt-") {
                    let c = rest.chars().next()?;
                    if rest.chars().count() == 1 && c.is_ascii_alphabetic() {
                        return Some(Key::Alt(c));
                    }
                    return None;
                }
                if let Some(rest) = n.strip_prefix('f') {
                    if let Ok(v) = rest.parse::<u8>() {
                        if (1..=12).contains(&v) {
                            return Some(Key::F(v));
                        }
                    }
                }
                if n.chars().count() == 1 {
                    return Some(Key::Char(n.chars().next()?));
                }
                return None;
            }
        })
    }

    /// The character this key would insert into a text field, if any.
    pub fn as_char(&self) -> Option<char> {
        match self {
            Key::Char(c) => Some(*c),
            _ => None,
        }
    }

    pub fn is_ctrl(&self, c: char) -> bool {
        matches!(self, Key::Ctrl(k) if *k == c)
    }
}

/// Keys as one integer, the way compiled programs (C, Go, assembly) see
/// them: printable characters are their Unicode value, named keys start
/// at `KEY_NAMED`, Ctrl and Alt combinations at `KEY_CTRL` and `KEY_ALT`.
pub const KEY_NAMED: i32 = 0x110000;
pub const KEY_CTRL: i32 = 0x120000;
pub const KEY_ALT: i32 = 0x130000;
/// A key event as one integer: the key code, plus this bit when released.
pub const KEY_UP_BIT: i32 = 0x100_0000;

impl Key {
    pub fn code(self) -> i32 {
        match self {
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

    /// The inverse of [`Key::code`].
    pub fn from_code(code: i32) -> Option<Key> {
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
}

impl KeyEvent {
    /// The key's code, with `KEY_UP_BIT` set when the key was released.
    pub fn code(self) -> i32 {
        self.key.code() | if self.down { 0 } else { KEY_UP_BIT }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names() {
        assert_eq!(Key::parse_name("enter"), Some(Key::Enter));
        assert_eq!(Key::parse_name("Ctrl-C"), Some(Key::Ctrl('c')));
        assert_eq!(Key::parse_name("f5"), Some(Key::F(5)));
        assert_eq!(Key::parse_name("a"), Some(Key::Char('a')));
        assert_eq!(Key::parse_name("ж"), Some(Key::Char('ж')));
        assert_eq!(Key::parse_name("nope"), None);
    }
}
