# sokoban

Levels as pictures, rules as a few lines of code. Also the clearest
example of strings used as a map.

## Levels

Each level is eight `DATA` strings, ten characters wide:

```basic
DATA "##########", "#        #", "#  @$  . #", "#        #", ...
```

`#` wall, `$` box, `.` target, `@` the player, space floor. Level `n`
is found by `RESTORE` and reading past `n * 8` strings; there is no
random access to `DATA`.

Loading splits each row into two strings: `rows(y)` holds walls, boxes
and floor, `targ(y)` holds only the targets. The player is taken out
of the map into `px, py`. Keeping targets separate is what lets a box be
pushed off a target again: the target is still in `targ()`.

## Moving is four lines

```basic
c$ = MID(rows(ny), nx, 1)
IF c$ = "#" THEN GOTO @play                   ' wall
IF c$ = "$" THEN                              ' box: can it move too?
    b$ = MID(rows(by), bx, 1)
    IF b$ <> " " THEN GOTO @play
    rows(by) = LEFT(rows(by), bx) + "$" + MID(rows(by), bx + 1)
    rows(ny) = LEFT(rows(ny), nx) + " " + MID(rows(ny), nx + 1)
END IF
px = nx: py = ny
```

That is the entire rule set of Sokoban: you can walk onto floor, you can
push one box into floor, nothing else. Changing a character inside a
string is `LEFT` + new char + `MID` from the next position (0-based).

## Winning

After each move, count boxes not standing on a target:

```basic
IF MID(rows(y), x, 1) = "$" AND MID(targ(y), x, 1) <> "." THEN todo = todo + 1
```

Zero means the level is done; the next level loads, and after the last
one the game says so and `END 0`. `R` restarts a level (`GOTO @load`),
ESC quits through `@quit`, which clears the screen first so the shell
gets a clean one back.

## Drawing

`@drawall` repaints the 10x8 map with `PUT` after every move, choosing a
symbol and color per cell: player white, walls gray, a box yellow or
green when it sits on a target, targets cyan. Two characters per cell
so the map is roughly square on an 8x16 font.

## Making levels

Draw one in the `DATA` lines: exactly 10 wide, 8 rows, one `@`, as many
`$` as `.`. Check it is solvable by hand: boxes cannot be pulled, so a
box pushed into a corner is stuck forever. Bump `levels`.
