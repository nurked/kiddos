# tetris

The largest BASIC cartridge (~190 lines) and the one that uses
subroutines. It shows pieces as data, a 2-D array as the board, and
`GOSUB` for everything done more than once.

## Pieces are pictures

Each of the seven pieces is four 16-character strings, one per
rotation, a 4x4 picture read left to right, top to bottom:

```basic
DATA "....XXXX........", "..X...X...X...X.", ...   ' the I piece
```

The 28 strings go into `shape()`; piece `p` at rotation `r` is
`shape(p * 4 + r)`, and cell `(r, c)` of it is `MID(s$, r * 4 + c, 1)`
(`MID` counts from 0). Rotating is `rot = (rot + 1) MOD 4`; there is no
rotation math anywhere. A kid can draw a new piece with dots and X's.

## The board

```basic
DIM board(10, 20) AS INTEGER     ' 0 empty, else the color it was locked with
```

Screen cells are two characters wide (`"[]"`), so board column `x` is
screen column `ox + x * 2`. Colors are `piece + 9`, the seven bright
CGA colors.

## Subroutines

The falling piece is never on the board; it is drawn over it. Four
subroutines do all the work, called with `GOSUB` and ending in `RETURN`:

| label | does |
|---|---|
| `@fits` | sets `ok`: does the piece fit at `px, py, rot`? (walls, floor, board) |
| `@draw` / `@erase` | `PUT` the four cells of the piece, or spaces |
| `@lock` | copy the piece into `board()`, find full lines, score, adjust speed |
| `@clearline` | pull every row above line `y` down one; called from `@lock` |
| `@redraw` | repaint the whole board from `board()` after a line clear |

Every move is the same shape: erase, change `px`/`py`/`rot`, `GOSUB
@fits`, undo if not `ok`, draw. A hard drop (`SPACE`) is `WHILE ok:
py = py + 1: GOSUB @fits: WEND` then one step back.

Gravity uses the clock, not the loop count: `IF TICK - lastdrop > speed`,
so the loop can poll keys with `INKEY$` and `SLEEP 0.02` while the fall
rate stays independent. `speed` shrinks 20 ms per cleared line, floor 100.

## Two BASIC rules learned here

Variables must be assigned before their first read *in source order*.
The main loop reads `ok`, which the subroutines at the bottom of the
file set, so the top of the file assigns `ok = TRUE`, `s$ = ""` and the
rest once.

Two sibling `FOR` loops nested inside another `FOR` crash the 0.12
compiler (duplicate exit labels). The line-clearing code has exactly
that shape, so it lives in `@clearline`, where its loops are top level.
