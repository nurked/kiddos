//! winit key events → the console's [`Key`] vocabulary.

use kiddos_console::Key;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};

/// The parent chord: Ctrl+Alt+Shift+P (Cmd counts as Ctrl on macOS).
pub fn is_parent_chord(event: &KeyEvent, mods: ModifiersState) -> bool {
    let ctrl = mods.control_key() || mods.super_key();
    if !(ctrl && mods.alt_key() && mods.shift_key()) {
        return false;
    }
    matches!(&event.logical_key, WKey::Character(c) if c.eq_ignore_ascii_case("p"))
        || matches!(
            event.physical_key,
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyP)
        )
}

pub fn map(event: &KeyEvent, mods: ModifiersState) -> Option<Key> {
    if event.state != ElementState::Pressed {
        return None;
    }
    let ctrl = mods.control_key();
    let alt = mods.alt_key();
    if let WKey::Named(n) = &event.logical_key {
        return Some(match n {
            NamedKey::Enter => Key::Enter,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Tab => {
                if mods.shift_key() {
                    Key::BackTab
                } else {
                    Key::Tab
                }
            }
            NamedKey::Escape => Key::Escape,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Insert => Key::Insert,
            NamedKey::Delete => Key::Delete,
            NamedKey::Space => Key::Char(' '),
            NamedKey::F1 => Key::F(1),
            NamedKey::F2 => Key::F(2),
            NamedKey::F3 => Key::F(3),
            NamedKey::F4 => Key::F(4),
            NamedKey::F5 => Key::F(5),
            NamedKey::F6 => Key::F(6),
            NamedKey::F7 => Key::F(7),
            NamedKey::F8 => Key::F(8),
            NamedKey::F9 => Key::F(9),
            NamedKey::F10 => Key::F(10),
            NamedKey::F11 => Key::F(11),
            NamedKey::F12 => Key::F(12),
            _ => return None,
        });
    }
    if ctrl || alt {
        // Ctrl+letter: use the physical key so Ctrl-C works in any layout
        if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
            let name = format!("{code:?}");
            if let Some(l) = name.strip_prefix("Key") {
                if l.len() == 1 {
                    let c = l.chars().next().unwrap().to_ascii_lowercase();
                    return Some(if ctrl { Key::Ctrl(c) } else { Key::Alt(c) });
                }
            }
        }
        return None;
    }
    match &event.logical_key {
        WKey::Character(s) => {
            let mut it = s.chars();
            let c = it.next()?;
            if it.next().is_some() || c.is_control() {
                return None;
            }
            Some(Key::Char(c))
        }
        _ => event
            .text
            .as_ref()
            .and_then(|t| t.chars().next())
            .filter(|c| !c.is_control())
            .map(Key::Char),
    }
}
