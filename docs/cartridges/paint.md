# paint

The pixel-mode showcase. A 64 x 36 picture of big dots, each drawn as a
5 x 5 pixel square, in the 16 CGA colors. About 150 lines of BASIC,
and the first program on the machine that draws instead of prints.

## Two screens, one program

```basic
SCREEN 13
GFX_SYNC FALSE
```

`SCREEN 13` switches to pixels (QBasic used the same number for the same
320 x 200, 256-color mode). `GFX_SYNC FALSE` turns off "show every
drawing call at once": from here on nothing appears until `GFX_FLIP`.
The main loop flips exactly once per key press, so the cursor never
flickers.

## The picture is an array

```basic
DIM pic(64, 36) AS INTEGER
```

`pic(x, y)` is the color of a dot, 0 for empty. Drawing a dot is a
subroutine, because it is needed in four places:

```basic
@dot
COLOR pic(px, py)
GFX_RECTF px * 5, py * 5, px * 5 + 4, py * 5 + 4
RETURN
```

`GFX_RECTF` takes two corners, both included, so a dot at `px` covers
pixels `px*5` to `px*5+4`. `COLOR` sets the drawing color for every
`GFX_` word that follows; in pixel mode it accepts 0 to 255, but Paint
sticks to the 16 a kid already knows from `COLOR`.

## The cursor

The cursor is a white outline (`GFX_RECT`) drawn over the current dot
right before the flip, and removed after the key by drawing the dot
again (`@uncursor` calls `@dot`). No saved-background trick is needed:
the array remembers what was there.

## Keys

`KEY` waits for one key and returns its name. Arrows move, `SPACE`
paints, `x` erases. Colors come from single characters: the loop checks
`LEN(k$) = 1`, takes `ASC(k$)` and maps `0`-`9`, `a`-`f`, `A`-`F` onto
0-15. Letters arrive as typed, so both cases are handled.

## Saving as text

```basic
row$ = row$ + CHR(64 + pic(i, j))    ' A = 1 ... O = 15
WRITEFILE "picture.txt", t$
```

The file is 36 lines of 64 characters, `.` for empty. Because it is
text, `cat picture.txt` shows the picture as letters, and `edit` can
change it; `L` reads it back with `READFILE` and walks the string with
`MID(t$, c, 1)` (0-based, like everything in this BASIC), starting a new
row at each `CHR(10)`.

## Things to change

The dot size (5) and the grid (64 x 36) are the first two numbers in the
file. Halving the dot size gives a 128 x 72 picture; the save format does
not care. The man page suggests it.
