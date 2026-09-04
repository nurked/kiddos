#!/bin/basic
' SNAKE. Arrows steer, ESC quits. Eat @ to grow.
' The body is remembered in two lists: bx() and by().

DIM bx(600) AS INTEGER
DIM by(600) AS INTEGER

CLS
FOR i = 0 TO 79
    PUT i, 1, "#", 8, 0
    PUT i, 24, "#", 8, 0
NEXT
FOR i = 1 TO 24
    PUT 0, i, "#", 8, 0
    PUT 79, i, "#", 8, 0
NEXT

x = 40
y = 12
dx = 1
dy = 0
length = 4
score = 0
FOR i = 0 TO length - 1
    bx(i) = x - (length - 1) + i
    by(i) = y
    PUT bx(i), by(i), "O", 10, 0
NEXT
head = length - 1

fx = INT(RND(1) * 76) + 2
fy = INT(RND(1) * 21) + 2
PUT fx, fy, "@", 12, 0
PUT 2, 0, "SNAKE   score: 0     arrows steer, ESC quits", 14, 0

DO
    k$ = INKEY$
    IF k$ = "UP" AND dy = 0 THEN
        dx = 0
        dy = -1
    END IF
    IF k$ = "DOWN" AND dy = 0 THEN
        dx = 0
        dy = 1
    END IF
    IF k$ = "LEFT" AND dx = 0 THEN
        dx = -1
        dy = 0
    END IF
    IF k$ = "RIGHT" AND dx = 0 THEN
        dx = 1
        dy = 0
    END IF
    IF k$ = "ESC" THEN EXIT DO

    x = x + dx
    y = y + dy
    IF x <= 0 OR x >= 79 OR y <= 1 OR y >= 24 THEN EXIT DO

    hit = FALSE
    FOR i = 0 TO length - 1
        j = (head - i + 600) MOD 600
        IF bx(j) = x AND by(j) = y THEN hit = TRUE
    NEXT
    IF hit THEN EXIT DO

    head = (head + 1) MOD 600
    bx(head) = x
    by(head) = y
    PUT x, y, "O", 10, 0

    IF x = fx AND y = fy THEN
        length = length + 1
        score = score + 10
        BEEP 880, 40
        PUT 2, 0, "SNAKE   score: " + LTRIM(STR(score)) + "   ", 14, 0
        fx = INT(RND(1) * 76) + 2
        fy = INT(RND(1) * 21) + 2
        PUT fx, fy, "@", 12, 0
    ELSE
        tail = (head - length + 600) MOD 600
        PUT bx(tail), by(tail), " ", 7, 0
    END IF

    SLEEP 0.08
LOOP

PUT 28, 12, "  GAME OVER   score: " + LTRIM(STR(score)) + "  ", 15, 4
PUT 28, 13, "  press a key to leave    ", 15, 4
SPEAK "Game over"
k$ = KEY$
CLS
END 0
