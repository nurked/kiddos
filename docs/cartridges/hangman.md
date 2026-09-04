# hangman

String handling. The game keeps two strings the same length as the
secret word and rebuilds one of them letter by letter.

## Words are DATA

```basic
DATA "elephant", "penguin", "computer", ...
n = INT(RND(1) * 10)
FOR i = 0 TO n
    READ word$
NEXT
```

There is no "read the n-th item", so it reads `n + 1` items and keeps
the last. `READ` needs a plain variable in this BASIC; that is why words
are not read into an array. Changing the vocabulary is editing the
`DATA` line and the `10`.

## The state is three strings and a number

- `word$` the secret
- `found$` what the kid sees: `_` for unknown letters
- `tried$` every letter pressed, shown so they stop repeating
- `wrong` counts misses; six draws the whole figure

A guess rebuilds `found$` from scratch:

```basic
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
```

Note `MID` counts from 0 in this version. Winning is simply
`found$ = word$`.

## Keys and case

`KEY$` returns a single character for letters, so the check is
`LEN(k$) = 1`. There is no `UCASE`, so uppercase input is folded with
`ASC`/`CHR`: `IF c >= 65 AND c <= 90 THEN k$ = CHR(c + 32)`. Non-letters
are ignored.

## Drawing

The gallows is five `PRINT` lines, each chosen by `wrong`. A line
ending in a backslash cannot be written as `"\"` because `\"` is an
escape in string literals; `+ CHR(92)` builds it. The whole screen is
redrawn with `CLS` every turn: at this size that is simpler than
updating cells, and the flash is part of the retro feel.
