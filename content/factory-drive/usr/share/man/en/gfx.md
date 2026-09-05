# gfx
> pixel mode: 320 x 200 dots in 256 colors, from BASIC, C and Go

## WHAT IT IS
The screen has two faces. Text mode is 80 x 25 letters. Pixel mode is
320 dots across and 200 down, and any dot can be one of 256 colors.
A program is in one or the other: the first drawing word switches to
pixels, and the text comes back when the program leaves pixel mode or
ends. In BASIC a picture stays on screen until you press a key.

Drawing is double-buffered: you draw on a hidden page and then flip it
onto the screen, so a game can draw a whole frame without flicker.

## COLORS
- 0-15: the same colors as `COLOR` (14 is yellow, 4 is red)
- 16-31: grays, from black to white
- 32-247: a cube of colors: `32 + 36*r + 6*g + b` with r, g, b from 0 to 5
- `PALETTE n, red, green, blue` changes what any number looks like

## BASIC
```
SCREEN 13                       ' pixels (SCREEN 0 is text again)
COLOR 14
GFX_CIRCLEF 160, 100, 40        ' filled circle at x, y with radius
GFX_LINE 0, 0, 319, 199
GFX_RECT 10, 10, 60, 40         ' outline from one corner to the other
GFX_RECTF 10, 150, 60, 190      ' filled
GFX_PIXEL 5, 5
GFX_TEXT 100, 20, "hello"       ' letters, 8 pixels each
GFX_SYNC FALSE                  ' stop showing every step...
GFX_FLIP                        ' ...and show them all at once
PRINT GFX_GET(5, 5)             ' the color at a dot
IF KEYDOWN("LEFT") THEN x = x - 1   ' true while the key is held
```

## C AND GO
```
kd_gfx_fill(0, 0, 320, 200, KD_RGB(0, 0, 3));   /* dark blue sky */
kd_gfx_circle(160, 60, 30, KD_YELLOW, 1);
kd_gfx_text(8, 8, "hello", KD_WHITE, -1);
kd_gfx_flip();
while (!kd_key_down(KD_KEY_ESC)) kd_sleep(16);
```
Go spells them `kiddos.GfxFill`, `kiddos.GfxCircle`, `kiddos.GfxText`,
`kiddos.GfxFlip`, `kiddos.KeyDown`. Both also have `gfx_blit` to copy a
block of dots (a sprite) with one see-through color, `gfx_read` to copy
dots out, and `key_event` for every key going down or up.

## TRY THIS
```
play paint
cp /usr/share/examples/bounce.c .
cc bounce.c && ./bounce.wasm
cat /usr/share/examples/sun.go
```

## SEE ALSO
basic, cc, goc, paint

## GROWN-UP NOTE
320 x 200 doubled is exactly the 640 x 400 text raster, so the renderer
uploads the pixel buffer through the same CRT shader. It is console API
v2; the text API is unchanged. Key state (`key_down`, `key_event`) is
tracked by the kernel from the host's press and release events and is
cleared when a program enters pixel mode.
