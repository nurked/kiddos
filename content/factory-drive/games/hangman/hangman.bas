#!/bin/basic
' HANGMAN. Guess the word one letter at a time. Six wrong and it is over.
DATA "elephant", "penguin", "computer", "keyboard", "rainbow", "dragon", "pyramid", "volcano", "bicycle", "sandwich"

n = INT(RND(1) * 10)
FOR i = 0 TO n
    READ word$
NEXT

found$ = ""
FOR i = 1 TO LEN(word$)
    found$ = found$ + "_"
NEXT
wrong = 0
tried$ = ""

DO
    CLS
    COLOR 14
    PRINT "  HANGMAN"
    COLOR 7
    PRINT
    PRINT "   +---+"
    line$ = "   |"
    IF wrong >= 1 THEN line$ = "   |   O"
    PRINT line$
    line$ = "   |"
    IF wrong = 2 THEN line$ = "   |   |"
    IF wrong = 3 THEN line$ = "   |  /|"
    IF wrong >= 4 THEN line$ = "   |  /|" + CHR(92)
    PRINT line$
    line$ = "   |"
    IF wrong = 5 THEN line$ = "   |  /"
    IF wrong >= 6 THEN line$ = "   |  / " + CHR(92)
    PRINT line$
    PRINT "  ===   wrong:"; wrong; "of 6"
    PRINT
    PRINT "  Word:  "; found$
    PRINT "  Tried: "; tried$
    PRINT
    IF found$ = word$ THEN
        COLOR 10
        PRINT "  YOU WIN! The word was "; word$
        COLOR 7
        SPEAK "You win"
        k$ = KEY$
        END 0
    END IF
    IF wrong >= 6 THEN
        COLOR 12
        PRINT "  Oh no. The word was "; word$
        COLOR 7
        SPEAK "Game over"
        k$ = KEY$
        END 1
    END IF
    PRINT "  Press a letter. ESC quits."
    k$ = KEY$
    IF k$ = "ESC" THEN END 1
    IF LEN(k$) = 1 THEN
        c = ASC(k$)
        IF c >= 65 AND c <= 90 THEN k$ = CHR(c + 32)
        c = ASC(k$)
        IF c >= 97 AND c <= 122 THEN
            tried$ = tried$ + k$
            hit = FALSE
            new$ = ""
            FOR i = 0 TO LEN(word$) - 1
                IF MID(word$, i, 1) = k$ THEN
                    new$ = new$ + k$
                    hit = TRUE
                ELSE
                    new$ = new$ + MID(found$, i, 1)
                END IF
            NEXT
            found$ = new$
            IF NOT hit THEN wrong = wrong + 1
            IF NOT hit THEN BEEP 200, 100
        END IF
    END IF
LOOP
