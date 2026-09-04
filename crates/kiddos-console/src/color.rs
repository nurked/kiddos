//! The 16-color CGA/EGA palette. Colors are palette indices everywhere in the
//! console API; only the renderer ever sees RGB.

pub type Rgb = [u8; 3];

/// Classic CGA palette (with the "brown" dark yellow).
pub const PALETTE: [Rgb; 16] = [
    [0x00, 0x00, 0x00], // 0 black
    [0x00, 0x00, 0xAA], // 1 blue
    [0x00, 0xAA, 0x00], // 2 green
    [0x00, 0xAA, 0xAA], // 3 cyan
    [0xAA, 0x00, 0x00], // 4 red
    [0xAA, 0x00, 0xAA], // 5 magenta
    [0xAA, 0x55, 0x00], // 6 brown
    [0xAA, 0xAA, 0xAA], // 7 light gray
    [0x55, 0x55, 0x55], // 8 dark gray
    [0x55, 0x55, 0xFF], // 9 light blue
    [0x55, 0xFF, 0x55], // 10 light green
    [0x55, 0xFF, 0xFF], // 11 light cyan
    [0xFF, 0x55, 0x55], // 12 light red
    [0xFF, 0x55, 0xFF], // 13 light magenta
    [0xFF, 0xFF, 0x55], // 14 yellow
    [0xFF, 0xFF, 0xFF], // 15 white
];

pub mod colors {
    pub const BLACK: u8 = 0;
    pub const BLUE: u8 = 1;
    pub const GREEN: u8 = 2;
    pub const CYAN: u8 = 3;
    pub const RED: u8 = 4;
    pub const MAGENTA: u8 = 5;
    pub const BROWN: u8 = 6;
    pub const LIGHT_GRAY: u8 = 7;
    pub const DARK_GRAY: u8 = 8;
    pub const LIGHT_BLUE: u8 = 9;
    pub const LIGHT_GREEN: u8 = 10;
    pub const LIGHT_CYAN: u8 = 11;
    pub const LIGHT_RED: u8 = 12;
    pub const LIGHT_MAGENTA: u8 = 13;
    pub const YELLOW: u8 = 14;
    pub const WHITE: u8 = 15;

    /// Default text color.
    pub const DEFAULT_FG: u8 = LIGHT_GRAY;
    /// Default background.
    pub const DEFAULT_BG: u8 = BLACK;
}

/// Names a kid can type: `color green`, `echo -c yellow`.
pub fn by_name(name: &str) -> Option<u8> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => 0,
        "blue" => 1,
        "green" => 2,
        "cyan" => 3,
        "red" => 4,
        "magenta" | "purple" => 5,
        "brown" | "orange" => 6,
        "gray" | "grey" | "lightgray" | "lightgrey" => 7,
        "darkgray" | "darkgrey" => 8,
        "lightblue" => 9,
        "lightgreen" | "lime" => 10,
        "lightcyan" => 11,
        "lightred" | "pink" => 12,
        "lightmagenta" => 13,
        "yellow" => 14,
        "white" => 15,
        _ => return None,
    })
}

pub const NAMES: [&str; 16] = [
    "black",
    "blue",
    "green",
    "cyan",
    "red",
    "magenta",
    "brown",
    "gray",
    "darkgray",
    "lightblue",
    "lightgreen",
    "lightcyan",
    "lightred",
    "lightmagenta",
    "yellow",
    "white",
];
