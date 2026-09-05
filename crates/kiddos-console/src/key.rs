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
