//! KidDOS statements: SPEAK, BEEP, KEY$, TICK, PUT. They mirror the console
//! API one to one.

use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, ExprType};
use endbasic_core::compiler::{ArgSepSyntax, RequiredValueSyntax, SingularArgSyntax};
use endbasic_core::exec::{Machine, Result, Scope};
use endbasic_core::syms::{Callable, CallableMetadata, CallableMetadataBuilder};
use kiddos_kernel::{Console, Proc};
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

const CATEGORY: &str = "KidDOS";

macro_rules! arg {
    ($name:literal, $t:ident, $sep:expr) => {
        SingularArgSyntax::RequiredValue(
            RequiredValueSyntax {
                name: Cow::Borrowed($name),
                vtype: ExprType::$t,
            },
            $sep,
        )
    };
}

pub fn add_all(machine: &mut Machine, p: Arc<Proc>) {
    machine.add_callable(Rc::new(Speak::new(p.clone())));
    machine.add_callable(Rc::new(Beep::new(p.clone())));
    machine.add_callable(Rc::new(KeyFn::new(p.clone())));
    machine.add_callable(Rc::new(Tick::new(p.clone())));
    machine.add_callable(Rc::new(Put::new(p)));
}

/// Turn EndBASIC's error text into something a kid can act on.
pub fn humanize(msg: &str) -> String {
    let m = msg.trim();
    let hint = if m.contains("Undefined variable") || m.contains("Undefined symbol") {
        " (Did you spell it the same way everywhere?)"
    } else if m.contains("Unknown command") || m.contains("Unknown builtin") {
        " (Type HELP to see the words BASIC knows.)"
    } else if m.contains("Division by zero") {
        " (Nothing can be split into zero parts, not even by a computer.)"
    } else if m.contains("expected") || m.contains("Expected") {
        " (Something is missing or in the wrong place on that line.)"
    } else {
        ""
    };
    format!("{m}{hint}")
}

struct Speak {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl Speak {
    fn new(p: Arc<Proc>) -> Speak {
        Speak {
            metadata: CallableMetadataBuilder::new("SPEAK")
                .with_syntax(&[(&[arg!("text", Text, ArgSepSyntax::End)], None)])
                .with_category(CATEGORY)
                .with_description("Says the text out loud through the machine's voice.")
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for Speak {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let text = scope.pop_string();
        self.p.speak(&text);
        Ok(())
    }
}

struct Beep {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl Beep {
    fn new(p: Arc<Proc>) -> Beep {
        Beep {
            metadata: CallableMetadataBuilder::new("BEEP")
                .with_syntax(&[
                    (&[], None),
                    (
                        &[
                            arg!("freq", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                            arg!("ms", Integer, ArgSepSyntax::End),
                        ],
                        None,
                    ),
                ])
                .with_category(CATEGORY)
                .with_description("Makes a sound. BEEP alone, or BEEP freq, milliseconds.")
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for Beep {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let (freq, ms) = if scope.nargs() == 0 {
            (880, 150)
        } else {
            let a = scope.pop_integer();
            let b = scope.pop_integer();
            (a, b)
        };
        self.p.beep(freq.max(0) as u32, ms.max(0) as u32);
        Ok(())
    }
}

struct KeyFn {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl KeyFn {
    fn new(p: Arc<Proc>) -> KeyFn {
        KeyFn {
            metadata: CallableMetadataBuilder::new("KEY")
                .with_return_type(ExprType::Text)
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Waits for a key and returns its name: a letter, or UP, DOWN, LEFT, RIGHT, ENTER, ESC, \
                     SPACE... Use INKEY$ if you do not want to wait.",
                )
                .build(),
            p,
        }
    }
}

pub fn key_name(k: kiddos_kernel::Key) -> String {
    use kiddos_kernel::Key as K;
    match k {
        K::Char(' ') => "SPACE".into(),
        K::Char(c) => c.to_string(),
        K::Enter => "ENTER".into(),
        K::Backspace => "BS".into(),
        K::Tab | K::BackTab => "TAB".into(),
        K::Escape => "ESC".into(),
        K::Up => "UP".into(),
        K::Down => "DOWN".into(),
        K::Left => "LEFT".into(),
        K::Right => "RIGHT".into(),
        K::Home => "HOME".into(),
        K::End => "END".into(),
        K::PageUp => "PGUP".into(),
        K::PageDown => "PGDOWN".into(),
        K::Delete => "DEL".into(),
        K::Insert => "INS".into(),
        K::F(n) => format!("F{n}"),
        K::Ctrl('c') => "INT".into(),
        K::Ctrl(c) => format!("CTRL-{}", c.to_ascii_uppercase()),
        K::Alt(c) => format!("ALT-{}", c.to_ascii_uppercase()),
    }
}

#[async_trait(?Send)]
impl Callable for KeyFn {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        match self.p.readkey() {
            Ok(k) => scope.return_string(key_name(k)),
            Err(_) => Err(scope.io_error(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"))),
        }
    }
}

struct Tick {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl Tick {
    fn new(p: Arc<Proc>) -> Tick {
        Tick {
            metadata: CallableMetadataBuilder::new("TICK")
                .with_return_type(ExprType::Integer)
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description("Milliseconds since the machine was turned on. For timing games.")
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for Tick {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        scope.return_integer((self.p.tick() & 0x7FFF_FFFF) as i32)
    }
}

struct Put {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl Put {
    fn new(p: Arc<Proc>) -> Put {
        Put {
            metadata: CallableMetadataBuilder::new("PUT")
                .with_syntax(&[(
                    &[
                        arg!("x", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("y", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("text", Text, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("fg", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("bg", Integer, ArgSepSyntax::End),
                    ],
                    None,
                )])
                .with_category(CATEGORY)
                .with_description(
                    "Draws text at a cell without moving the cursor: PUT x, y, \"text\", fg, bg. \
                     Colors are 0-15 like COLOR.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for Put {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let x = scope.pop_integer();
        let y = scope.pop_integer();
        let text = scope.pop_string();
        let fg = scope.pop_integer();
        let bg = scope.pop_integer();
        let to_cga = |n: i32| -> u8 { (n.max(0) % 16) as u8 };
        for (i, ch) in text.chars().enumerate() {
            self.p.put(
                (x.max(0) as usize + i) as u16,
                y.max(0) as u16,
                ch,
                to_cga(fg),
                to_cga(bg),
            );
        }
        Ok(())
    }
}
