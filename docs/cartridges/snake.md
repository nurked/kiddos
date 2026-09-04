# snake

The console-API showcase: a real-time loop that polls the keyboard,
moves one cell, and draws with `PUT`. About 90 lines.

## The body is a ring

The interesting data structure. The snake is stored in two arrays,
`bx()` and `by()`, used as a ring buffer of 600 cells:

```basic
DIM bx(600) AS INTEGER
DIM by(600) AS INTEGER
head = length - 1
```

Each step, `head` moves forward (`(head + 1) MOD 600`) and the new head
cell is stored there. The tail is `(head - length + 600) MOD 600`: not
stored anywhere, computed. Growing is `length = length + 1`, which means
"don't erase the tail this step". Collision with itself walks back
`length` cells from the head:

```basic
FOR i = 0 TO length - 1
    j = (head - i + 600) MOD 600
    IF bx(j) = x AND by(j) = y THEN hit = TRUE
NEXT
```

`+ 600` before the `MOD` keeps it positive; BASIC's `MOD` of a negative
number is negative.

## The loop

```basic
DO
    k$ = INKEY$                 ' "" if nothing pressed; never waits
    IF k$ = "UP" AND dy = 0 THEN ...   ' no reversing into yourself
    x = x + dx: y = y + dy
    ' walls, self, then move head, draw, eat or erase tail
    SLEEP 0.08
LOOP
```

`INKEY$` rather than `KEY$` is what makes it real-time: the snake keeps
moving whether or not a key is pressed. The `dy = 0` guard on UP/DOWN
(and `dx = 0` on LEFT/RIGHT) is the classic rule that you cannot turn
180° in one step.

Drawing never clears the screen. Each step draws one head cell and
erases one tail cell with `PUT ... " "`. The border and the score line
are drawn once. This is why it does not flicker.

## Speed and score

`SLEEP 0.08` is the whole difficulty setting; the man page tells the kid
to copy the file and change it. Eating beeps (`BEEP 880, 40`) and
rewrites the score line with `LTRIM(STR(score))` (plain `STR` puts a
space in front of the number).

## Ending

Walls or self-collision `EXIT DO`, then a boxed GAME OVER is `PUT` on top
of the board, the machine says so, and `KEY$` waits before `CLS` and
`END 0` hand a clean screen back to the shell.
