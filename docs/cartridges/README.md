# Cartridges

A cartridge is a folder under `/games` on the virtual drive. Adding one
never needs a recompile: the machine reads the folder at run time. This
page is the format; the other pages in this directory walk through each
shipped game and how its code is written.

## The folder

```
/games/snake/
├── cart.toml        # who and what; required
├── snake.bas        # the entry: what `play snake` runs
├── README.md        # shown by `play snake --about`, and cat-able
└── man/snake.md     # picked up by `man snake` automatically
```

Anything else (text files, levels, ASCII art, badge files) can sit in the
folder; the entry reaches them through `$CART` (shell) or an absolute
`/games/<name>/...` path (BASIC).

## cart.toml

```toml
name = "snake"                 # folder name; lowercase, digits, dashes
title = "Snake"                # shown by `games`
version = "0.1.0"
author = "KidDOS"
entry = "snake.bas"            # file to run, relative to the folder
description = "steer the snake, eat the @, don't bite yourself"
caps = ["speak", "sound"]      # what the game may do: speak, sound
world = ["~/cave"]             # folders where the kid "is inside the game"
memory_mb = 64                 # compiled entries: more than the default 16 MB
```

`entry` is usually a file in the folder. It may also be the name of a
command built into the machine (vi-quest and prison-escape do this);
then the folder holds only docs and data. A file entry runs through the
kernel exactly like a file the kid would run, so it needs an executable
bit and a shebang: `#!/bin/basic` for BASIC,
`#!/bin/ksh` for a shell script. A `.wasm` entry needs no shebang: the
kernel recognises the file and runs it in the sandbox. `install` sets the bit on any file that
starts with `#!`.

`memory_mb` (optional) raises the sandbox's 16 MB memory cap for a
compiled entry; Doom asks for 64, the ceiling is 256.

`caps` is the capability list from the plan: a cartridge without `speak`
cannot talk, without `sound` cannot beep. `world` tells the tutor to keep
quiet while the kid's current folder is inside one of those paths (the
adventure lives in `~/cave`).

## What the entry gets

- Environment: `CART=/games/<name>`, `GAME=<name>`, plus the kid's usual
  `HOME`, `USER`, `PATH`.
- The console API, identically in BASIC and shell: the screen, the keys,
  `speak`/`SPEAK`, `beep`/`BEEP`, the clock.
- The drive: `/games` is read-only (root-owned). To let the kid change
  things, copy them into `~` first, as the adventure does.
- The exit code: `END 0` in BASIC or the last command's status in a
  script. `guess` uses it: its `main.sh` copies a badge only on `END 0`.

## Writing one

Kids: `newgame rocket` scaffolds a working BASIC cartridge in `~/rocket`.
Grown-ups adding one to the factory drive: create the folder under
`content/factory-drive/games/`, `chmod +x` the entry, rebuild. The build
script bakes `content/factory-drive` into the binary.

## Sharing: `.kdc`

A `.kdc` is a plain zip of the folder. Parent mode:

```
share /home/kid/rocket     # writes carts/rocket.kdc on the real computer
carts                      # lists that folder and the installed games
install rocket             # unpacks carts/rocket.kdc into /games/rocket
uninstall rocket
```

`carts/` sits beside the drive file (`~/Library/Application Support/KidDOS/`
on a Mac) and is the only place files cross between the fake machine and
the real one. Cartridges are unsigned for now and say so on install.

## BASIC, as shipped

The BASIC is EndBASIC 0.12 (pinned for its Apache license). The KidDOS
additions every game below uses:

| word | does |
|---|---|
| `PUT x, y, "text", fg, bg` | draw text at a cell without moving the cursor; colors 0–15, CGA numbering (14 yellow, 1 blue) |
| `KEY$` | wait for a key, return its name: a letter, `SPACE`, `ENTER`, `ESC`, `UP`... |
| `INKEY$` | the key pressed right now or `""`; never waits |
| `TICK` | milliseconds since boot |
| `SPEAK "..."`, `BEEP freq, ms` | voice and sound |
| `SCREEN 13`, `GFX_RECTF x1, y1, x2, y2`, `GFX_TEXT x, y, "t"`, `GFX_FLIP`, `KEYDOWN("UP")` | pixel mode, 320x200 in 256 colors; see `man gfx` |
| `READFILE("f")`, `WRITEFILE "f", t$`, `APPENDFILE "f", t$` | files, as text |

Quirks of this EndBASIC version, learned the hard way and documented in
[../ARCHITECTURE.md](../ARCHITECTURE.md): `MID` counts from 0, `STR`
adds a leading space (wrap it in `LTRIM`), `READ` needs a plain variable,
a variable must be assigned before it is first read in source order, and
two sibling `FOR` loops nested inside another `FOR` crash the compiler
(put them in a `GOSUB` subroutine).

## The games

| page | teaches | written in |
|---|---|---|
| [adventure](adventure.md) | ls, cd, cat, mv, ls -a, grep, ./script, chmod | shell + text files |
| [guess](guess.md) | the first program to read and change | BASIC, 30 lines |
| [snake](snake.md) | a real-time loop, INKEY$, PUT, arrays as a ring | BASIC |
| [hangman](hangman.md) | strings, DATA, building text one letter at a time | BASIC |
| [typing](typing.md) | KEY$, TICK, measuring time | BASIC |
| [tetris](tetris.md) | 2-D arrays, GOSUB subroutines, pictures as data | BASIC |
| [sokoban](sokoban.md) | levels as pictures, rules as code, strings as maps | BASIC |
| [paint](paint.md) | pixel mode: GFX_ words, one flip per key, a picture saved as text | BASIC |
| [rogue](rogue.md) | a real program in C: arrays, structure, a game loop with no libc | C → wasm |
| [doom](doom.md) | a famous real program ported: a platform layer, a libc, a memory cap; installed, not built in | C + wasi-libc → wasm |
| [prison-escape](prison-escape.md) | how to get out of vi: `:q`, `:q!`, `:wq` | Rust, on the vi engine |
| [vi-quest](vi-quest.md) | vi's motions and edits, one land at a time; unlocks `vi` | Rust + TOML levels |
