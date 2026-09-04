#!/bin/basic
' TYPING PRACTICE. Type each line exactly, then Enter. ESC quits.
DATA "cat", "ls", "cd games", "pwd", "cat sign", "mkdir box", "echo hi", "cd ..", "man ls", "play snake"

CLS
COLOR 14
PRINT "  TYPING PRACTICE"
COLOR 7
PRINT "  Type each line exactly, then press Enter. ESC quits."
PRINT "  Press any key to start."
k$ = KEY$
start = TICK
mistakes = 0

FOR w = 1 TO 10
    READ word$
    PRINT
    COLOR 15
    PRINT "  "; word$
    COLOR 7
    PRINT "  ";
    typed$ = ""
    DO
        k$ = KEY$
        IF k$ = "ESC" THEN END 1
        IF k$ = "ENTER" THEN EXIT DO
        IF k$ = "SPACE" THEN k$ = " "
        IF LEN(k$) = 1 THEN
            typed$ = typed$ + k$
            PRINT k$;
        END IF
    LOOP
    IF typed$ <> word$ THEN
        mistakes = mistakes + 1
        COLOR 12
        PRINT "   (it was: "; word$; ")"
        COLOR 7
    ELSE
        ' a little chime for a good line; a wrong line gets silence
        BEEP 660, 50
        BEEP 990, 80
        COLOR 10
        PRINT "   ok"
        COLOR 7
    END IF
NEXT

ms = TICK - start
IF ms < 1 THEN ms = 1
wpm = 10 * 60000 / ms
PRINT
PRINT "  Time:"; ms / 1000; "seconds.   Mistakes:"; mistakes
COLOR 10
PRINT "  Speed:"; wpm; "words per minute."
COLOR 7
SPEAK "Done. " + LTRIM(STR(wpm)) + " words per minute."
PRINT "  Press a key."
k$ = KEY$
END 0
