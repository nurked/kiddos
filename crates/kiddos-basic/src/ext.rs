//! KidDOS statements: SPEAK, BEEP, KEY$, TICK, PUT, the file words
//! READFILE, WRITEFILE, APPENDFILE, and for pixel mode SCREEN, PALETTE,
//! GFX_TEXT, GFX_FLIP, GFX_GET, KEYDOWN. They mirror the console API one
//! to one.

use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, ExprType};
use endbasic_core::compiler::{ArgSepSyntax, RequiredValueSyntax, SingularArgSyntax};
use endbasic_core::exec::{Machine, Result, Scope};
use endbasic_core::syms::{Callable, CallableMetadata, CallableMetadataBuilder};
use endbasic_std::console::Console as EbConsole;
use kiddos_kernel::{Console, Proc};
use std::borrow::Cow;
use std::cell::RefCell;
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

pub fn add_all(machine: &mut Machine, p: Arc<Proc>, console: Rc<RefCell<dyn EbConsole>>) {
    machine.add_callable(Rc::new(Speak::new(p.clone())));
    machine.add_callable(Rc::new(Beep::new(p.clone())));
    machine.add_callable(Rc::new(KeyFn::new(p.clone())));
    machine.add_callable(Rc::new(Tick::new(p.clone())));
    machine.add_callable(Rc::new(Put::new(p.clone())));
    machine.add_callable(Rc::new(ScreenMode::new(p.clone())));
    machine.add_callable(Rc::new(Palette::new(p.clone())));
    machine.add_callable(Rc::new(GfxText::new(p.clone(), console)));
    machine.add_callable(Rc::new(GfxFlip::new(p.clone())));
    machine.add_callable(Rc::new(GfxGet::new(p.clone())));
    machine.add_callable(Rc::new(KeyDown::new(p.clone())));
    machine.add_callable(Rc::new(ReadFile::new(p.clone())));
    machine.add_callable(Rc::new(WriteFile::new(p.clone(), false)));
    machine.add_callable(Rc::new(WriteFile::new(p, true)));
}

/// The key a name from [`key_name`] stands for (case-insensitive).
pub fn key_from_name(name: &str) -> Option<kiddos_kernel::Key> {
    use kiddos_kernel::Key as K;
    let n = name.trim().to_ascii_uppercase();
    Some(match n.as_str() {
        "SPACE" => K::Char(' '),
        "ENTER" => K::Enter,
        "BS" | "BACKSPACE" => K::Backspace,
        "TAB" => K::Tab,
        "ESC" | "ESCAPE" => K::Escape,
        "UP" => K::Up,
        "DOWN" => K::Down,
        "LEFT" => K::Left,
        "RIGHT" => K::Right,
        "HOME" => K::Home,
        "END" => K::End,
        "PGUP" => K::PageUp,
        "PGDOWN" | "PGDN" => K::PageDown,
        "DEL" => K::Delete,
        "INS" => K::Insert,
        _ => {
            if let Some(f) = n.strip_prefix('F').and_then(|d| d.parse::<u8>().ok()) {
                return (1..=12).contains(&f).then_some(K::F(f));
            }
            let mut it = name.trim().chars();
            let c = it.next()?;
            if it.next().is_some() {
                return None;
            }
            // letters are held as the lowercase key; KEYDOWN("A") means the A key
            K::Char(c.to_lowercase().next().unwrap_or(c))
        }
    })
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

struct ScreenMode {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl ScreenMode {
    fn new(p: Arc<Proc>) -> ScreenMode {
        ScreenMode {
            metadata: CallableMetadataBuilder::new("SCREEN")
                .with_syntax(&[(&[arg!("mode", Integer, ArgSepSyntax::End)], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Picks the screen mode: SCREEN 13 is pixels (320 x 200, 256 colors), SCREEN 0 is \
                     text again. GFX_ statements switch to pixels on their own; when a program ends in \
                     pixel mode the picture stays until a key is pressed.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for ScreenMode {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let mode = scope.pop_integer();
        self.p.gfx_mode(mode != 0);
        if mode != 0 {
            self.p.gfx_flip();
        }
        Ok(())
    }
}

struct Palette {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl Palette {
    fn new(p: Arc<Proc>) -> Palette {
        Palette {
            metadata: CallableMetadataBuilder::new("PALETTE")
                .with_syntax(&[(
                    &[
                        arg!("color", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("red", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("green", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("blue", Integer, ArgSepSyntax::End),
                    ],
                    None,
                )])
                .with_category(CATEGORY)
                .with_description(
                    "Changes what a color number looks like in pixel mode: PALETTE color, red, green, blue \
                     with each of red, green, blue from 0 to 255. Colors 0-15 start as the COLOR colors, \
                     16-31 as grays, 32-247 as a rainbow cube: 32 + 36*r + 6*g + b with r, g, b from 0 to 5.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for Palette {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let i = scope.pop_integer();
        let r = scope.pop_integer();
        let g = scope.pop_integer();
        let b = scope.pop_integer();
        let ch = |v: i32| v.clamp(0, 255) as u8;
        self.p.gfx_palette(ch(i), [ch(r), ch(g), ch(b)]);
        Ok(())
    }
}

struct GfxText {
    metadata: CallableMetadata,
    p: Arc<Proc>,
    console: Rc<RefCell<dyn EbConsole>>,
}

impl GfxText {
    fn new(p: Arc<Proc>, console: Rc<RefCell<dyn EbConsole>>) -> GfxText {
        GfxText {
            metadata: CallableMetadataBuilder::new("GFX_TEXT")
                .with_syntax(&[(
                    &[
                        arg!("x", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("y", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("text", Text, ArgSepSyntax::End),
                    ],
                    None,
                )])
                .with_category(CATEGORY)
                .with_description(
                    "Writes text in pixel mode at pixel x, y with the current COLOR, 8 pixels per letter. \
                     The background shows through.",
                )
                .build(),
            p,
            console,
        }
    }
}

#[async_trait(?Send)]
impl Callable for GfxText {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let x = scope.pop_integer();
        let y = scope.pop_integer();
        let text = scope.pop_string();
        let (fg, _) = self.console.borrow().color();
        if !self.p.gfx_on() {
            self.p.gfx_mode(true);
        }
        self.p
            .gfx_text(x, y, &text, fg.unwrap_or(kiddos_console::colors::DEFAULT_FG), None);
        self.p.gfx_flip();
        Ok(())
    }
}

struct GfxFlip {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl GfxFlip {
    fn new(p: Arc<Proc>) -> GfxFlip {
        GfxFlip {
            metadata: CallableMetadataBuilder::new("GFX_FLIP")
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Shows everything drawn since the last flip. Use GFX_SYNC FALSE first so drawing is \
                     hidden until then: that is how games animate without flicker.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for GfxFlip {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, _scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        if !self.p.gfx_on() {
            self.p.gfx_mode(true);
        }
        self.p.gfx_flip();
        Ok(())
    }
}

struct GfxGet {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl GfxGet {
    fn new(p: Arc<Proc>) -> GfxGet {
        GfxGet {
            metadata: CallableMetadataBuilder::new("GFX_GET")
                .with_return_type(ExprType::Integer)
                .with_syntax(&[(
                    &[
                        arg!("x", Integer, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("y", Integer, ArgSepSyntax::End),
                    ],
                    None,
                )])
                .with_category(CATEGORY)
                .with_description("The color number of the pixel at x, y (0 outside the screen).")
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for GfxGet {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let x = scope.pop_integer();
        let y = scope.pop_integer();
        scope.return_integer(self.p.gfx_get(x, y) as i32)
    }
}

struct KeyDown {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl KeyDown {
    fn new(p: Arc<Proc>) -> KeyDown {
        KeyDown {
            metadata: CallableMetadataBuilder::new("KEYDOWN")
                .with_return_type(ExprType::Boolean)
                .with_syntax(&[(&[arg!("name", Text, ArgSepSyntax::End)], None)])
                .with_category(CATEGORY)
                .with_description(
                    "TRUE while a key is held down: KEYDOWN(\"LEFT\"), KEYDOWN(\"SPACE\"), KEYDOWN(\"A\"). \
                     For games where holding a key keeps you moving. KEY and INKEY$ report presses instead.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for KeyDown {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let name = scope.pop_string();
        let held = key_from_name(&name).map(|k| self.p.key_held(k)).unwrap_or(false);
        scope.return_boolean(held)
    }
}

struct ReadFile {
    metadata: CallableMetadata,
    p: Arc<Proc>,
}

impl ReadFile {
    fn new(p: Arc<Proc>) -> ReadFile {
        ReadFile {
            metadata: CallableMetadataBuilder::new("READFILE")
                .with_return_type(ExprType::Text)
                .with_syntax(&[(&[arg!("path", Text, ArgSepSyntax::End)], None)])
                .with_category(CATEGORY)
                .with_description(
                    "The whole text of a file: T$ = READFILE(\"notes.txt\"). Lines are separated by \
                     CHR(10). An empty string if there is no such file.",
                )
                .build(),
            p,
        }
    }
}

#[async_trait(?Send)]
impl Callable for ReadFile {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let path = scope.pop_string();
        let text = self.p.fs().read_string(&path).unwrap_or_default();
        scope.return_string(text)
    }
}

struct WriteFile {
    metadata: CallableMetadata,
    p: Arc<Proc>,
    append: bool,
}

impl WriteFile {
    fn new(p: Arc<Proc>, append: bool) -> WriteFile {
        let (name, desc) = if append {
            (
                "APPENDFILE",
                "Adds text to the end of a file: APPENDFILE \"log.txt\", \"one more line\" + CHR(10).",
            )
        } else {
            (
                "WRITEFILE",
                "Puts text into a file, replacing what was there: WRITEFILE \"notes.txt\", T$. \
                 The shell can cat it afterwards.",
            )
        };
        WriteFile {
            metadata: CallableMetadataBuilder::new(name)
                .with_syntax(&[(
                    &[
                        arg!("path", Text, ArgSepSyntax::Exactly(ArgSep::Long)),
                        arg!("text", Text, ArgSepSyntax::End),
                    ],
                    None,
                )])
                .with_category(CATEGORY)
                .with_description(desc)
                .build(),
            p,
            append,
        }
    }
}

#[async_trait(?Send)]
impl Callable for WriteFile {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }
    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        let path = scope.pop_string();
        let text = scope.pop_string();
        let r = if self.append {
            self.p.fs().append(&path, text.as_bytes())
        } else {
            self.p.fs().write(&path, text.as_bytes())
        };
        match r {
            Ok(()) => Ok(()),
            Err(e) => Err(scope.io_error(std::io::Error::other(self.p.explain(&e)))),
        }
    }
}
