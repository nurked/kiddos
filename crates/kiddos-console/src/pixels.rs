//! Pixel mode: a 320x200 canvas with a 256-color palette, double-buffered.
//!
//! Programs draw into the back buffer and call [`Pixels::flip`] to show it.
//! Doubled, 320x200 is exactly the 640x400 text raster, so the renderer
//! shows one or the other with the same texture. Colors are palette
//! indices; only the renderer ever sees RGB.

use crate::color::{Rgb, PALETTE};
use crate::font::glyph;

pub const WIDTH: u16 = 320;
pub const HEIGHT: u16 = 200;

/// The default palette: the 16 CGA colors, 16 grays, then a 6x6x6 color
/// cube (index `32 + 36*r + 6*g + b` with r, g, b in 0..6) and 8 spares.
pub fn default_palette() -> [Rgb; 256] {
    let mut p = [[0u8; 3]; 256];
    p[..16].copy_from_slice(&PALETTE);
    for (i, e) in p[16..32].iter_mut().enumerate() {
        let v = (i * 17) as u8;
        *e = [v, v, v];
    }
    for i in 0..216 {
        let (r, g, b) = (i / 36, (i / 6) % 6, i % 6);
        p[32 + i] = [(r * 51) as u8, (g * 51) as u8, (b * 51) as u8];
    }
    // spares: a few useful in-betweens
    p[248] = [0xFF, 0x80, 0x00]; // orange
    p[249] = [0xFF, 0xC0, 0xCB]; // pink
    p[250] = [0x80, 0x40, 0x00]; // dark brown
    p[251] = [0x40, 0x80, 0x40]; // moss
    p[252] = [0x20, 0x20, 0x40]; // night
    p[253] = [0xFF, 0xE0, 0x80]; // sand
    p[254] = [0x00, 0x40, 0x80]; // deep blue
    p[255] = [0xE0, 0xE0, 0xFF]; // ice
    p
}

/// Nearest palette entry (default palette) for an RGB triple, for programs
/// that think in RGB (image import, Doom's own palette).
pub fn nearest(palette: &[Rgb; 256], rgb: Rgb) -> u8 {
    let mut best = 0usize;
    let mut best_d = u32::MAX;
    for (i, p) in palette.iter().enumerate() {
        let d = (0..3).map(|c| (p[c] as i32 - rgb[c] as i32).pow(2) as u32).sum();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best as u8
}

#[derive(Debug, Clone)]
pub struct Pixels {
    back: Vec<u8>,
    front: Vec<u8>,
    palette: [Rgb; 256],
    /// Bumps on `flip` and palette changes: the renderer polls it.
    generation: u64,
}

impl Default for Pixels {
    fn default() -> Self {
        Self::new()
    }
}

impl Pixels {
    pub fn new() -> Pixels {
        let n = WIDTH as usize * HEIGHT as usize;
        Pixels {
            back: vec![0; n],
            front: vec![0; n],
            palette: default_palette(),
            generation: 1,
        }
    }

    pub fn width(&self) -> u16 {
        WIDTH
    }
    pub fn height(&self) -> u16 {
        HEIGHT
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// What is on screen (the last flipped frame).
    pub fn front(&self) -> &[u8] {
        &self.front
    }
    /// What is being drawn (not visible until `flip`).
    pub fn back(&self) -> &[u8] {
        &self.back
    }
    pub fn palette(&self) -> &[Rgb; 256] {
        &self.palette
    }

    /// Show the back buffer. The back buffer keeps its contents, so a
    /// program may draw incrementally.
    pub fn flip(&mut self) {
        self.front.copy_from_slice(&self.back);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_palette(&mut self, i: u8, rgb: Rgb) {
        self.palette[i as usize] = rgb;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn reset_palette(&mut self) {
        self.palette = default_palette();
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn clear(&mut self, c: u8) {
        self.back.fill(c);
    }

    /// The back buffer's pixel, or 0 outside.
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            0
        } else {
            self.back[y as usize * WIDTH as usize + x as usize]
        }
    }

    #[inline]
    pub fn pixel(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            self.back[y as usize * WIDTH as usize + x as usize] = c;
        }
    }

    /// Bresenham line, both ends included.
    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, c: u8) {
        let (dx, dy) = ((x2 - x1).abs(), -(y2 - y1).abs());
        let (sx, sy) = (if x1 < x2 { 1 } else { -1 }, if y1 < y2 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x1, y1, dx + dy);
        // bounded: a line can be at most this long on the canvas
        for _ in 0..(dx - dy + 1).max(1) {
            self.pixel(x, y, c);
            if x == x2 && y == y2 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Outline of a `w` x `h` rectangle whose top-left is `(x, y)`.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (x2, y2) = (x + w - 1, y + h - 1);
        self.line(x, y, x2, y, c);
        self.line(x, y2, x2, y2, c);
        self.line(x, y, x, y2, c);
        self.line(x2, y, x2, y2, c);
    }

    /// Filled rectangle.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(WIDTH as i32);
        let y1 = (y + h).min(HEIGHT as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let start = row as usize * WIDTH as usize;
            self.back[start + x0 as usize..start + x1 as usize].fill(c);
        }
    }

    /// Midpoint circle, outline or filled.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: u8, filled: bool) {
        if r < 0 {
            return;
        }
        if r == 0 {
            self.pixel(cx, cy, c);
            return;
        }
        let (mut x, mut y, mut d) = (r, 0, 1 - r);
        while x >= y {
            if filled {
                self.line(cx - x, cy + y, cx + x, cy + y, c);
                self.line(cx - x, cy - y, cx + x, cy - y, c);
                self.line(cx - y, cy + x, cx + y, cy + x, c);
                self.line(cx - y, cy - x, cx + y, cy - x, c);
            } else {
                for (px, py) in [
                    (cx + x, cy + y),
                    (cx - x, cy + y),
                    (cx + x, cy - y),
                    (cx - x, cy - y),
                    (cx + y, cy + x),
                    (cx - y, cy + x),
                    (cx + y, cy - x),
                    (cx - y, cy - x),
                ] {
                    self.pixel(px, py, c);
                }
            }
            y += 1;
            if d < 0 {
                d += 2 * y + 1;
            } else {
                x -= 1;
                d += 2 * (y - x) + 1;
            }
        }
    }

    /// Copy a `w` x `h` block of palette indices (row-major, `w` bytes per
    /// row) to `(x, y)`. Pixels equal to `transparent` are skipped. Short
    /// data is treated as the rows it does contain.
    pub fn blit(&mut self, x: i32, y: i32, w: i32, h: i32, data: &[u8], transparent: Option<u8>) {
        if w <= 0 || h <= 0 {
            return;
        }
        let rows = (data.len() / w as usize).min(h as usize);
        for row in 0..rows as i32 {
            let py = y + row;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let src = &data[row as usize * w as usize..][..w as usize];
            let dst_row = py as usize * WIDTH as usize;
            for (i, &c) in src.iter().enumerate() {
                let px = x + i as i32;
                if px < 0 || px >= WIDTH as i32 || transparent == Some(c) {
                    continue;
                }
                self.back[dst_row + px as usize] = c;
            }
        }
    }

    /// Copy a block out of the back buffer (row-major, `w` bytes per row).
    /// Pixels outside the canvas read as 0.
    pub fn read(&self, x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
        let mut out = Vec::with_capacity((w.max(0) * h.max(0)) as usize);
        for row in 0..h.max(0) {
            for col in 0..w.max(0) {
                out.push(self.get(x + col, y + row));
            }
        }
        out
    }

    /// Draw text with the 8x8 font, `bg == None` leaves the background as
    /// it is. Returns the x after the last glyph.
    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: u8, bg: Option<u8>) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for (gy, bits) in g.iter().enumerate() {
                for gx in 0..8 {
                    if bits & (1 << gx) != 0 {
                        self.pixel(cx + gx, y + gy as i32, fg);
                    } else if let Some(b) = bg {
                        self.pixel(cx + gx, y + gy as i32, b);
                    }
                }
            }
            cx += 8;
        }
        cx
    }

    /// Width in pixels of `s` when drawn with [`Pixels::text`].
    pub fn text_width(s: &str) -> i32 {
        s.chars().count() as i32 * 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_layout() {
        let p = default_palette();
        assert_eq!(p[4], [0xAA, 0, 0]);
        assert_eq!(p[16], [0, 0, 0]);
        assert_eq!(p[31], [255, 255, 255]);
        assert_eq!(p[32], [0, 0, 0]);
        assert_eq!(p[32 + 215], [255, 255, 255]);
        assert_eq!(p[32 + 36 * 5], [255, 0, 0]);
        assert_eq!(nearest(&p, [250, 3, 2]), 32 + 36 * 5);
    }

    #[test]
    fn draws_and_flips() {
        let mut px = Pixels::new();
        px.pixel(5, 5, 7);
        assert_eq!(px.get(5, 5), 7);
        assert_eq!(px.front()[5 * 320 + 5], 0);
        px.flip();
        assert_eq!(px.front()[5 * 320 + 5], 7);
        px.pixel(-1, 0, 9);
        px.pixel(320, 0, 9);
        px.pixel(0, 200, 9);
        assert_eq!(px.get(-1, 0), 0);
    }

    #[test]
    fn shapes() {
        let mut px = Pixels::new();
        px.line(0, 0, 9, 9, 1);
        for i in 0..10 {
            assert_eq!(px.get(i, i), 1);
        }
        px.line(-50, 100, 400, 100, 2);
        assert_eq!(px.get(0, 100), 2);
        assert_eq!(px.get(319, 100), 2);
        px.rect(10, 10, 5, 4, 3);
        assert_eq!(px.get(10, 10), 3);
        assert_eq!(px.get(14, 13), 3);
        assert_eq!(px.get(12, 11), 0);
        px.fill(20, 20, 3, 3, 4);
        assert_eq!(px.get(22, 22), 4);
        assert_eq!(px.get(23, 22), 0);
        px.fill(-10, -10, 15, 15, 5);
        assert_eq!(px.get(0, 0), 5);
        assert_eq!(px.get(4, 4), 5);
        assert_eq!(px.get(5, 6), 0);
        px.circle(100, 150, 10, 6, false);
        assert_eq!(px.get(110, 150), 6);
        assert_eq!(px.get(100, 140), 6);
        assert_eq!(px.get(100, 150), 0);
        px.circle(200, 150, 10, 7, true);
        assert_eq!(px.get(200, 150), 7);
        assert_eq!(px.get(209, 150), 7);
    }

    #[test]
    fn blit_and_text() {
        let mut px = Pixels::new();
        let sprite = [1, 0, 1, 0, 1, 0];
        px.blit(0, 0, 3, 2, &sprite, Some(0));
        assert_eq!(px.get(0, 0), 1);
        assert_eq!(px.get(1, 0), 0);
        px.fill(0, 0, 3, 2, 9);
        px.blit(0, 0, 3, 2, &sprite, Some(0));
        assert_eq!(px.get(1, 0), 9, "transparent keeps what was there");
        px.blit(0, 0, 3, 2, &sprite, None);
        assert_eq!(px.get(1, 0), 0);
        assert_eq!(px.read(0, 0, 3, 2), sprite);
        let end = px.text(10, 10, "Hi", 15, None);
        assert_eq!(end, 26);
        let lit = (0..16)
            .flat_map(|x| (0..8).map(move |y| (x, y)))
            .filter(|(x, y)| px.get(10 + x, 10 + y) == 15)
            .count();
        assert!(lit > 10);
    }
}
