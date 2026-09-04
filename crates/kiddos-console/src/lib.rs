//! The console is the only thing a KidDOS program ever sees.
//!
//! * [`Screen`] is the 80x25 (configurable) cell buffer with a cursor and an
//!   ANSI-subset writer.
//! * [`Key`] is the keyboard vocabulary.
//! * [`Console`] is the API contract. It is versioned by [`API_VERSION`] and is
//!   exposed identically to Rust builtins, BASIC and WASM.

pub mod color;
pub mod cyrillic;
pub mod font;
pub mod key;
pub mod screen;

pub use color::{colors, Rgb, PALETTE};
pub use key::Key;
pub use screen::{Cell, Screen};

/// Version of the console API contract. Bump only on breaking changes.
pub const API_VERSION: u32 = 1;

/// Default grid.
pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 25;

/// Returned when the process was told to stop (Ctrl-C, `kill`, shutdown)
/// while it was blocked in the console. Programs should unwind and exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interrupted;

impl std::fmt::Display for Interrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("interrupted")
    }
}
impl std::error::Error for Interrupted {}

/// The contract. One API, three bindings (Rust, BASIC, WASM).
///
/// `print`/`eprint`/`readline` go through the process's stdin/stdout/stderr
/// streams (which may be pipes or files). Everything else talks to the
/// terminal directly and is only meaningful for interactive programs.
pub trait Console {
    /// (cols, rows)
    fn size(&self) -> (u16, u16);
    /// Put one character at an absolute cell. Does not move the cursor.
    fn put(&self, x: u16, y: u16, ch: char, fg: u8, bg: u8);
    /// Write text at the cursor to stdout. Handles `\n`, scrolling and an
    /// ANSI SGR subset when stdout is the terminal.
    fn print(&self, s: &str);
    /// Write text to stderr.
    fn eprint(&self, s: &str);
    /// Move the cursor.
    fn cursor(&self, x: u16, y: u16);
    /// Current cursor position.
    fn cursor_pos(&self) -> (u16, u16);
    fn cursor_show(&self, visible: bool);
    /// Clear the whole screen to `bg` and home the cursor.
    fn clear(&self, bg: u8);
    /// Set the colors used by subsequent `print`.
    fn set_color(&self, fg: u8, bg: u8);
    /// Non-blocking key read.
    fn getkey(&self) -> Option<Key>;
    /// Blocking key read.
    fn readkey(&self) -> Result<Key, Interrupted>;
    /// Read a line from stdin. `Ok(None)` means end of input.
    /// When stdin is the terminal this echoes and supports Backspace.
    fn readline(&self, prompt: &str) -> Result<Option<String>, Interrupted>;
    /// Sleep, yielding to the machine. Returns early with `Interrupted`.
    fn sleep(&self, ms: u64) -> Result<(), Interrupted>;
    /// Milliseconds since boot.
    fn tick(&self) -> u64;
    /// Square-wave beep. Rate limited by the kernel.
    fn beep(&self, freq: u32, ms: u32);
    /// Text to speech in the current language. Capability gated: returns
    /// `false` if the process may not speak.
    fn speak(&self, text: &str) -> bool;
    /// Was this process asked to stop? Long loops should poll this.
    fn interrupted(&self) -> bool;
    /// Is stdout the terminal (as opposed to a pipe or file)?
    fn stdout_is_tty(&self) -> bool;
    /// Is stdin the terminal?
    fn stdin_is_tty(&self) -> bool;
}
