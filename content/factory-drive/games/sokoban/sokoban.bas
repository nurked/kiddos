#!/bin/basic
' SOKOBAN. Push every box ($) onto a target (.). Arrows move, R restarts, ESC quits.
' Levels are pictures: # wall, $ box, . target, @ you. Ten wide, eight tall.
' (MID counts letters from 0 in this BASIC.)
DATA "##########", "#        #", "#  @$  . #", "#        #", "#        #", "#        #", "#        #", "##########"
DATA "##########", "#        #", "#  $  .  #", "#        #", "#  .  $  #", "#        #", "#        #", "#@       #"
DATA "##########", "#   #    #", "# $ #  . #", "#   #    #", "#        #", "#  #  $  #", "#  # .   #", "#@       #"
levels = 3

DIM rows(8) AS STRING
DIM targ(8) AS STRING
level = 0

@load
RESTORE
FOR i = 0 TO level * 8 - 1
    READ skip$
NEXT
px = 0
py = 0
FOR y = 0 TO 7
    READ r$
    rows(y) = r$
    t$ = ""
    m$ = ""
    FOR x = 0 TO 9
        c$ = MID(rows(y), x, 1)
        IF c$ = "." OR c$ = "*" OR c$ = "+" THEN
            t$ = t$ + "."
        ELSE
            t$ = t$ + " "
        END IF
        IF c$ = "@" OR c$ = "+" THEN
            px = x
            py = y
            c$ = " "
        END IF
        IF c$ = "*" THEN c$ = "$"
        IF c$ = "." THEN c$ = " "
        m$ = m$ + c$
    NEXT
    targ(y) = t$
    rows(y) = m$
NEXT
moves = 0
CLS
PUT 2, 0, "SOKOBAN   level " + LTRIM(STR(level + 1)) + " of " + LTRIM(STR(levels)) + "   arrows move, R restarts, ESC quits", 14, 0
GOSUB @drawall

@play
k$ = KEY$
IF k$ = "ESC" THEN GOTO @quit
IF k$ = "r" OR k$ = "R" THEN GOTO @load
dx = 0
dy = 0
IF k$ = "UP" THEN dy = -1
IF k$ = "DOWN" THEN dy = 1
IF k$ = "LEFT" THEN dx = -1
IF k$ = "RIGHT" THEN dx = 1
IF dx = 0 AND dy = 0 THEN GOTO @play
nx = px + dx
ny = py + dy
c$ = MID(rows(ny), nx, 1)
IF c$ = "#" THEN GOTO @play
IF c$ = "$" THEN
    bx = nx + dx
    by = ny + dy
    b$ = MID(rows(by), bx, 1)
    IF b$ <> " " THEN GOTO @play
    rows(by) = LEFT(rows(by), bx) + "$" + MID(rows(by), bx + 1)
    rows(ny) = LEFT(rows(ny), nx) + " " + MID(rows(ny), nx + 1)
END IF
px = nx
py = ny
moves = moves + 1
GOSUB @drawall
todo = 0
FOR y = 0 TO 7
    FOR x = 0 TO 9
        IF MID(rows(y), x, 1) = "$" AND MID(targ(y), x, 1) <> "." THEN todo = todo + 1
    NEXT
NEXT
IF todo > 0 THEN GOTO @play
BEEP 880, 100
PUT 30, 10, " LEVEL DONE in " + LTRIM(STR(moves)) + " moves ", 15, 2
SPEAK "Level done"
k$ = KEY$
level = level + 1
IF level < levels THEN GOTO @load
CLS
PRINT
PRINT "  You solved every level. That is real thinking."
SPEAK "You solved every level"
k$ = KEY$
CLS
END 0

@quit
CLS
END 1

@drawall
FOR y = 0 TO 7
    FOR x = 0 TO 9
        c$ = MID(rows(y), x, 1)
        t$ = MID(targ(y), x, 1)
        sx = 30 + x * 2
        sy = 4 + y
        IF x = px AND y = py THEN
            PUT sx, sy, "@ ", 15, 0
        ELSEIF c$ = "#" THEN
            PUT sx, sy, "##", 8, 0
        ELSEIF c$ = "$" AND t$ = "." THEN
            PUT sx, sy, "[]", 10, 0
        ELSEIF c$ = "$" THEN
            PUT sx, sy, "[]", 14, 0
        ELSEIF t$ = "." THEN
            PUT sx, sy, ". ", 11, 0
        ELSE
            PUT sx, sy, "  ", 7, 0
        END IF
    NEXT
NEXT
PUT 30, 13, "moves: " + LTRIM(STR(moves)) + "   ", 7, 0
RETURN
