#!/bin/basic
' TETRIS. LEFT/RIGHT move, UP turns, DOWN hurries, SPACE drops, ESC quits.
' Each piece is four 4x4 pictures, one per turn. X is a block.
DATA "....XXXX........", "..X...X...X...X.", "....XXXX........", "..X...X...X...X."
DATA "XX..XX..........", "XX..XX..........", "XX..XX..........", "XX..XX.........."
DATA ".X..XXX.........", ".X..XX...X......", "....XXX..X......", ".X...XX..X......"
DATA "X...XXX.........", ".XX..X...X......", "....XXX...X.....", ".X...X..XX......"
DATA "..X.XXX.........", ".X...X...XX.....", "....XXX.X.......", "XX...X...X......"
DATA ".XX.XX..........", ".X...XX...X.....", ".XX.XX..........", ".X...XX...X....."
DATA "XX...XX.........", "..X..XX..X......", "XX...XX.........", "..X..XX..X......"

DIM shape(28) AS STRING
FOR i = 0 TO 27
    READ s$
    shape(i) = s$
NEXT
DIM board(10, 20) AS INTEGER

ox = 30
oy = 2
CLS
FOR r = 0 TO 20
    PUT ox - 1, oy + r, "|", 8, 0
    PUT ox + 20, oy + r, "|", 8, 0
NEXT
FOR c = -1 TO 20
    PUT ox + c, oy + 20, "-", 8, 0
NEXT
PUT ox + 24, oy, "TETRIS", 14, 0
PUT ox + 24, oy + 2, "score 0", 7, 0
PUT ox + 24, oy + 4, "arrows move, up turns", 8, 0
PUT ox + 24, oy + 5, "space drops", 8, 0
PUT ox + 24, oy + 6, "esc quits", 8, 0
score = 0
cleared = 0
speed = 600
' variables the subroutines below share (BASIC wants them defined first)
ok = TRUE
s$ = ""
bx = 0
by = 0
full = FALSE
lastdrop = 0
k$ = ""

@newpiece
piece = INT(RND(1) * 7)
rot = 0
px = 3
py = 0
GOSUB @fits
IF NOT ok THEN GOTO @done
GOSUB @draw
lastdrop = TICK

@loop
k$ = INKEY$
IF k$ = "ESC" THEN GOTO @done
IF k$ = "LEFT" THEN
    GOSUB @erase
    px = px - 1
    GOSUB @fits
    IF NOT ok THEN px = px + 1
    GOSUB @draw
END IF
IF k$ = "RIGHT" THEN
    GOSUB @erase
    px = px + 1
    GOSUB @fits
    IF NOT ok THEN px = px - 1
    GOSUB @draw
END IF
IF k$ = "UP" THEN
    GOSUB @erase
    rot = (rot + 1) MOD 4
    GOSUB @fits
    IF NOT ok THEN rot = (rot + 3) MOD 4
    GOSUB @draw
END IF
IF k$ = "SPACE" THEN
    GOSUB @erase
    ok = TRUE
    WHILE ok
        py = py + 1
        GOSUB @fits
    WEND
    py = py - 1
    GOSUB @draw
    GOSUB @lock
    GOTO @newpiece
END IF
IF k$ = "DOWN" OR TICK - lastdrop > speed THEN
    lastdrop = TICK
    GOSUB @erase
    py = py + 1
    GOSUB @fits
    IF ok THEN
        GOSUB @draw
    ELSE
        py = py - 1
        GOSUB @draw
        GOSUB @lock
        GOTO @newpiece
    END IF
END IF
SLEEP 0.02
GOTO @loop

@done
PUT ox + 3, oy + 9, " GAME OVER ", 15, 4
PUT ox + 3, oy + 10, " score " + LTRIM(STR(score)) + " ", 15, 4
SPEAK "Game over"
k$ = KEY$
CLS
END 0

' --- does the piece fit at px, py, rot? sets ok ---
@fits
ok = TRUE
s$ = shape(piece * 4 + rot)
FOR r = 0 TO 3
    FOR c = 0 TO 3
        IF MID(s$, r * 4 + c, 1) = "X" THEN
            bx = px + c
            by = py + r
            IF bx < 0 OR bx > 9 OR by > 19 THEN ok = FALSE
            IF ok AND by >= 0 THEN
                IF board(bx, by) <> 0 THEN ok = FALSE
            END IF
        END IF
    NEXT
NEXT
RETURN

@draw
s$ = shape(piece * 4 + rot)
FOR r = 0 TO 3
    FOR c = 0 TO 3
        IF MID(s$, r * 4 + c, 1) = "X" THEN PUT ox + (px + c) * 2, oy + py + r, "[]", piece + 9, 0
    NEXT
NEXT
RETURN

@erase
s$ = shape(piece * 4 + rot)
FOR r = 0 TO 3
    FOR c = 0 TO 3
        IF MID(s$, r * 4 + c, 1) = "X" THEN PUT ox + (px + c) * 2, oy + py + r, "  ", 7, 0
    NEXT
NEXT
RETURN

' --- freeze the piece into the board, clear full lines ---
@lock
s$ = shape(piece * 4 + rot)
FOR r = 0 TO 3
    FOR c = 0 TO 3
        IF MID(s$, r * 4 + c, 1) = "X" THEN
            IF py + r >= 0 THEN board(px + c, py + r) = piece + 9
        END IF
    NEXT
NEXT
FOR y = 0 TO 19
    full = TRUE
    FOR x = 0 TO 9
        IF board(x, y) = 0 THEN full = FALSE
    NEXT
    IF full THEN GOSUB @clearline
NEXT
score = score + 10
PUT ox + 24, oy + 2, "score " + LTRIM(STR(score)) + "  lines " + LTRIM(STR(cleared)), 7, 0
speed = 600 - cleared * 20
IF speed < 100 THEN speed = 100
RETURN

' --- line y is full: pull everything above it down one row ---
@clearline
cleared = cleared + 1
score = score + 100
BEEP 660, 60
FOR yy = y TO 1 STEP -1
    FOR x = 0 TO 9
        board(x, yy) = board(x, yy - 1)
    NEXT
NEXT
FOR x = 0 TO 9
    board(x, 0) = 0
NEXT
GOSUB @redraw
RETURN

@redraw
FOR y = 0 TO 19
    FOR x = 0 TO 9
        IF board(x, y) = 0 THEN
            PUT ox + x * 2, oy + y, "  ", 7, 0
        ELSE
            PUT ox + x * 2, oy + y, "[]", board(x, y), 0
        END IF
    NEXT
NEXT
RETURN
