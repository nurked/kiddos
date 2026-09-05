#!/bin/basic
' PAINT. A 64 x 36 picture, each dot 5 x 5 screen pixels, in 16 colors.
' Arrows move, SPACE paints, X erases, 0-9 and A-F pick a hue,
' S saves ~/picture.txt (cat it!), L loads it, N is a new picture, ESC quits.

DIM pic(64, 36) AS INTEGER
W = 64
H = 36
cx = 32
cy = 18
hue = 14
k$ = ""
i = 0
j = 0
t$ = ""
row$ = ""
ch$ = ""
c = 0
px = 0
py = 0
saved = FALSE

SCREEN 13
GFX_SYNC FALSE
GOSUB @drawall

DO
    GOSUB @cursor
    GFX_FLIP
    k$ = KEY
    GOSUB @uncursor
    IF k$ = "ESC" THEN EXIT DO
    IF k$ = "LEFT" AND cx > 0 THEN cx = cx - 1
    IF k$ = "RIGHT" AND cx < W - 1 THEN cx = cx + 1
    IF k$ = "UP" AND cy > 0 THEN cy = cy - 1
    IF k$ = "DOWN" AND cy < H - 1 THEN cy = cy + 1
    IF k$ = "SPACE" THEN
        pic(cx, cy) = hue
        px = cx: py = cy: GOSUB @dot
    END IF
    IF k$ = "x" OR k$ = "X" THEN
        pic(cx, cy) = 0
        px = cx: py = cy: GOSUB @dot
    END IF
    IF LEN(k$) = 1 THEN
        c = ASC(k$)
        IF c >= 48 AND c <= 57 THEN hue = c - 48: GOSUB @bar
        IF c >= 97 AND c <= 102 THEN hue = c - 87: GOSUB @bar
        IF c >= 65 AND c <= 70 THEN hue = c - 55: GOSUB @bar
    END IF
    IF k$ = "s" OR k$ = "S" THEN GOSUB @save
    IF k$ = "l" OR k$ = "L" THEN GOSUB @load
    IF k$ = "n" OR k$ = "N" THEN
        FOR j = 0 TO H - 1
            FOR i = 0 TO W - 1
                pic(i, j) = 0
            NEXT
        NEXT
        GOSUB @drawall
    END IF
LOOP

SCREEN 0
PRINT "Bye! Your picture is in picture.txt if you pressed S."
END 0

' ---- one dot of the picture, 5 x 5 pixels ------------------------------
@dot
COLOR pic(px, py)
GFX_RECTF px * 5, py * 5, px * 5 + 4, py * 5 + 4
RETURN

' ---- the cursor: a white box around the dot, then put the dot back ----
@cursor
COLOR 15
GFX_RECT cx * 5, cy * 5, cx * 5 + 4, cy * 5 + 4
RETURN

@uncursor
px = cx: py = cy: GOSUB @dot
RETURN

' ---- the hue bar and help line at the bottom -------------------------
@bar
FOR i = 0 TO 15
    COLOR i
    GFX_RECTF i * 12, 182, i * 12 + 10, 190
    IF i = hue THEN
        COLOR 15
        GFX_RECT i * 12 - 1, 181, i * 12 + 11, 191
    END IF
NEXT
COLOR 0
GFX_RECTF 0, 192, 319, 199
COLOR 7
GFX_TEXT 0, 192, "arrows SPACE X 0-9 A-F  S save L load N new ESC"
RETURN

@drawall
COLOR 0
GFX_RECTF 0, 0, 319, 199
FOR j = 0 TO H - 1
    FOR i = 0 TO W - 1
        IF pic(i, j) <> 0 THEN
            px = i: py = j: GOSUB @dot
        END IF
    NEXT
NEXT
GOSUB @bar
RETURN

' ---- save: one line per row, "." for empty, A-O for colors 1-15 --------
@save
t$ = ""
FOR j = 0 TO H - 1
    row$ = ""
    FOR i = 0 TO W - 1
        IF pic(i, j) = 0 THEN
            row$ = row$ + "."
        ELSE
            row$ = row$ + CHR(64 + pic(i, j))
        END IF
    NEXT
    t$ = t$ + row$ + CHR(10)
NEXT
WRITEFILE "picture.txt", t$
BEEP 1200, 60
RETURN

@load
t$ = READFILE("picture.txt")
IF LEN(t$) = 0 THEN
    BEEP 200, 200
    RETURN
END IF
i = 0
j = 0
FOR c = 0 TO LEN(t$) - 1
    ch$ = MID(t$, c, 1)
    IF ch$ = CHR(10) THEN
        j = j + 1
        i = 0
    ELSE
        IF i < W AND j < H THEN
            IF ch$ = "." THEN
                pic(i, j) = 0
            ELSE
                pic(i, j) = ASC(ch$) - 64
            END IF
        END IF
        i = i + 1
    END IF
NEXT
GOSUB @drawall
BEEP 900, 60
RETURN
