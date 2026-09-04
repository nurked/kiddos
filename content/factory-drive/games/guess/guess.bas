' GUESS THE NUMBER
' The machine thinks of a number. You guess it.
' Copy me and change me:  cp /games/guess/guess.bas ~/mygame.bas

CLS
COLOR 14
PRINT "  GUESS THE NUMBER"
COLOR 7
PRINT
PRINT "  I am thinking of a number from 1 to 100."
PRINT "  Type a guess and press Enter. I will say higher or lower."
PRINT

secret = INT(RND(1) * 100) + 1
tries = 0

DO
    INPUT "  Your guess"; guess%
    tries = tries + 1
    IF guess% < secret THEN
        PRINT "  Higher!"
    ELSEIF guess% > secret THEN
        PRINT "  Lower!"
    ELSE
        COLOR 10
        PRINT "  YES! You got it in"; tries; "tries."
        COLOR 7
        SPEAK "You got it"
        EXIT DO
    END IF
LOOP

END 0
