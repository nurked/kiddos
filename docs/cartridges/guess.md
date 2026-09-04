# guess — Guess the Number

The first program a kid is meant to *read*. Lesson 12 has them copy it
(`cp /games/guess/guess.bas mygame.bas`) and change the 100 to 10.
Everything in it is chosen to be readable top to bottom.

## Files

```
/games/guess/
├── cart.toml     entry = "main.sh"
├── main.sh       basic $CART/guess.bas && cp badge → ~/badges/guess.txt
├── guess.bas
└── badge.txt
```

The entry is a two-line shell script rather than the BASIC file, so that
a win (`END 0`) earns a badge and a quit (`Ctrl-C`, exit 130) does not.
That is also a quiet demonstration that shell and BASIC compose.

## The program

```basic
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
        PRINT "  YES! You got it in"; tries; "tries."
        SPEAK "You got it"
        EXIT DO
    END IF
LOOP
END 0
```

Points of the design:

- `guess%` has the integer suffix on purpose. `INPUT` into a typed
  variable re-asks on bad input ("Retry input") instead of crashing when
  the kid types a word. `secret` and `tries` are untyped because they are
  never read from the keyboard.
- `DO ... EXIT DO ... LOOP` rather than `GOTO`: it is the shape of every
  game loop in the other cartridges, met here first in its simplest form.
- `IF / ELSEIF / ELSE / END IF` as a block, one branch per line, so the
  kid sees the three outcomes as three lines.
- The number range, the words, the colors are all on their own lines so
  each is a one-token change.
