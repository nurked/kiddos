# basic
> BASIC: write real programs (basic, or basic file.bas)

## WHAT IT DOES
BASIC is a programming language made for beginners in 1964, and it is
still the friendliest way to start. `basic` opens it. Type a line and
press Enter to run it right away. Type EXIT to come back to the shell.

## TRY THIS
```
basic
PRINT "hello"
PRINT 6 * 7
FOR i = 1 TO 5: PRINT i: NEXT
SPEAK "I can talk"
HELP
EXIT
```

## PROGRAMS
Inside BASIC, EDIT opens a program editor (Esc leaves it). RUN runs the
program, LIST shows it, SAVE "name" keeps it as name.bas in your home
folder, LOAD "name" brings it back. From the shell, `run name.bas` runs
it directly, and `cat name.bas` shows it like any file.

## KIDDOS WORDS
- `SPEAK "text"` talk
- `BEEP` or `BEEP 440, 200` sound
- `KEY$` wait for a key and give its name (UP, DOWN, A, SPACE...)
- `INKEY$` the key pressed right now, or "" (does not wait)
- `TICK` milliseconds since the machine started
- `PUT x, y, "text", fg, bg` draw text at a spot, in colors 0-15
- `CLS`, `COLOR fg, bg`, `LOCATE row, col`, `SLEEP seconds`, `INPUT`
- `READFILE("name")`, `WRITEFILE "name", text`, `APPENDFILE "name", text`
- `SCREEN 13` pixel mode, then `GFX_PIXEL`, `GFX_LINE`, `GFX_RECT`,
  `GFX_RECTF`, `GFX_CIRCLE`, `GFX_CIRCLEF`, `GFX_TEXT`, `GFX_FLIP`,
  `GFX_GET`, `PALETTE`, `KEYDOWN("LEFT")`: see `man gfx`

## SEE ALSO
run, edit, games, gfx

## GROWN-UP NOTE
This is EndBASIC 0.12 (Apache-2.0), a modern BASIC: no line numbers,
IF/ELSE/END IF, WHILE/WEND, DO/LOOP, FOR/NEXT, SELECT CASE, functions
and subroutines. Programs are plain files in the kid's home.
