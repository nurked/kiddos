//! Bitmap glyphs. The shipped font is the public-domain 8x8 set from the
//! `font8x8` crate plus our own Cyrillic block; the renderer doubles it
//! vertically into 8x16 cells for the classic look. `figlet` uses it too.

use font8x8::{UnicodeFonts, BASIC_FONTS, BLOCK_FONTS, BOX_FONTS, GREEK_FONTS, LATIN_FONTS, MISC_FONTS};

/// 8 rows, bit 0 = leftmost pixel. Unknown characters get a hollow box.
pub fn glyph(c: char) -> [u8; 8] {
    if let Some(g) = crate::cyrillic::get(c) {
        return g;
    }
    BASIC_FONTS
        .get(c)
        .or_else(|| LATIN_FONTS.get(c))
        .or_else(|| BOX_FONTS.get(c))
        .or_else(|| BLOCK_FONTS.get(c))
        .or_else(|| GREEK_FONTS.get(c))
        .or_else(|| MISC_FONTS.get(c))
        .or_else(|| special(c))
        .unwrap_or(UNKNOWN)
}

const UNKNOWN: [u8; 8] = [0x7E, 0x42, 0x42, 0x42, 0x42, 0x42, 0x7E, 0x00];

/// A few characters the shell and man pages use that font8x8 lacks.
fn special(c: char) -> Option<[u8; 8]> {
    Some(match c {
        '—' | '–' => [0, 0, 0, 0xFF, 0xFF, 0, 0, 0],
        '…' => [0, 0, 0, 0, 0, 0, 0x92, 0],
        '•' => [0, 0, 0x18, 0x3C, 0x3C, 0x18, 0, 0],
        '✓' => [0, 0x40, 0x60, 0x30, 0x1B, 0x0E, 0x04, 0],
        '☺' => [0x3C, 0x42, 0xA5, 0x81, 0xA5, 0x99, 0x42, 0x3C],
        '★' => [0x10, 0x10, 0x7C, 0x38, 0x38, 0x6C, 0x44, 0],
        '♥' => [0x00, 0x66, 0xFF, 0xFF, 0x7E, 0x3C, 0x18, 0],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::glyph;

    #[test]
    fn covers_expected_ranges() {
        assert_ne!(glyph('A'), super::UNKNOWN);
        assert_ne!(glyph('Ж'), super::UNKNOWN);
        assert_ne!(glyph('└'), super::UNKNOWN);
        assert_ne!(glyph('█'), super::UNKNOWN);
        assert_eq!(glyph('\u{1F600}'), super::UNKNOWN);
    }
}
