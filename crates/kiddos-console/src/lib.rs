//! The console is the only thing a KidDOS program ever sees.
//!
//! * [`Screen`] is the 80x25 (configurable) cell buffer with a cursor and an
//!   ANSI-subset writer.
//! * [`Key`] is the keyboard vocabulary.
//! * [`Pixels`] is the 320x200, 256-color canvas of pixel mode.
//! * [`Console`] is the API contract. It is versioned by [`API_VERSION`] and is
//!   exposed identically to Rust builtins, BASIC and WASM.

pub mod color;
pub mod cyrillic;
pub mod font;
pub mod key;
pub mod pixels;
pub mod screen;

pub use color::{colors, Rgb, PALETTE};
pub use key::{Key, KeyEvent};
pub use pixels::Pixels;
pub use screen::{Cell, Screen};

/// Version of the console API contract. Bump only on breaking changes.
pub const API_VERSION: u32 = 2;

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

    // ---- pixel mode (API v2) ------------------------------------------
    // 320x200 pixels, 256 colors, double-buffered. Exclusive with text:
    // while it is on, the text cells stay as they were but are not shown.
    // Drawing goes to the back buffer; `gfx_flip` shows it. Leaving pixel
    // mode (or the process ending) brings the text screen back.

    /// Enter (`true`) or leave (`false`) pixel mode. Every drawing call
    /// below enters it on its own, so `gfx_mode(true)` is only needed to
    /// show a black screen before the first frame.
    fn gfx_mode(&self, on: bool);
    fn gfx_on(&self) -> bool;
    /// Fill the back buffer with one color.
    fn gfx_clear(&self, c: u8);
    fn gfx_pixel(&self, x: i32, y: i32, c: u8);
    /// The back buffer's color at a point (0 outside the canvas).
    fn gfx_get(&self, x: i32, y: i32) -> u8;
    fn gfx_line(&self, x1: i32, y1: i32, x2: i32, y2: i32, c: u8);
    /// Outline of a `w` x `h` rectangle with top-left `(x, y)`.
    fn gfx_rect(&self, x: i32, y: i32, w: i32, h: i32, c: u8);
    /// Filled rectangle.
    fn gfx_fill(&self, x: i32, y: i32, w: i32, h: i32, c: u8);
    fn gfx_circle(&self, cx: i32, cy: i32, r: i32, c: u8, filled: bool);
    /// Copy a block of palette indices (`w` bytes per row) to `(x, y)`,
    /// skipping pixels equal to `transparent`.
    fn gfx_blit(&self, x: i32, y: i32, w: i32, h: i32, data: &[u8], transparent: Option<u8>);
    /// Copy a block out of the back buffer.
    fn gfx_read(&self, x: i32, y: i32, w: i32, h: i32) -> Vec<u8>;
    /// Change one palette entry. Takes effect on screen at once.
    fn gfx_palette(&self, i: u8, rgb: Rgb);
    /// Draw text with the 8x8 font; `bg == None` keeps the background.
    /// Returns the x after the last glyph.
    fn gfx_text(&self, x: i32, y: i32, s: &str, fg: u8, bg: Option<u8>) -> i32;
    /// Show the back buffer.
    fn gfx_flip(&self);

    // ---- key state (API v2) --------------------------------------------

    /// Is this key held down right now? Games use it to hold a direction.
    fn key_held(&self, k: Key) -> bool;
    /// Next key down/up event, if any. Presses also reach `readkey`.
    fn key_event(&self) -> Option<KeyEvent>;
}
