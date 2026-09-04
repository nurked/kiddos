# adventure — The Drive Below

The plan's "Bashcrawl model": a text adventure whose map *is* the
filesystem. Rooms are folders, things are files, signs are files you
`cat`, levers are scripts you run. The kid learns every Phase 0 command
without a lesson mentioning it.

## Files

```
/games/adventure/
├── cart.toml            world = ["~/cave"]  ← the tutor is quiet in there
├── main.sh              entry: builds the cave, prints the intro
├── intro.txt
├── rooms/               the cave, copied to ~/cave on first play
│   ├── sign, torch
│   └── tunnel/
│       ├── sign
│       ├── pit/  warning, .note
│       └── hall/ sign, book, statue/{sign,key}, door/{sign,open}
└── extra/               things that appear during play
    ├── locked.txt, opened.txt
    ├── badge.txt
    └── treasury/ sign, chest, wall, finish
```

## How it works

**`main.sh` builds the cave once.** `/games` is read-only, and the whole
point is that the kid moves and deletes things, so the rooms are copied
into the kid's home:

```sh
ls ~/cave > /dev/null 2> /dev/null || cp -r $CART/rooms ~/cave
```

`ksh` has no `if`, so `||` is the conditional: "if `ls` fails, copy".
Playing again keeps the kid's progress; `rm -r ~/cave` restarts.

**Every room has a `sign`.** The sign is the whole UI. It names the
things in the room in capitals (TORCH, TUNNEL) so `ls` output matches
what the sign said, and it always ends with the exact command to type
next. Nothing is hidden from `ls` except what is meant to be found with
`ls -a`.

**Carrying is `mv`.** "Take the torch with you: `mv torch tunnel`". The
torch has no game logic; it is a file whose presence the next sign asks
you to check with `ls`. The key is the one item that matters.

**The book teaches `grep`.** 70 lines of lore, one of which says where
the key is. The sign suggests `less` first (paging) and then
`grep key book`. The clue line contains lowercase `key`, because the
kid's grep is case-sensitive; the earlier version had it in capitals and
the test caught it.

**Levers are scripts.** `door/open` is the whole conditional logic of
the game, and it is one line of shell:

```sh
ls key > /dev/null 2> /dev/null && cp -r /games/adventure/extra/treasury treasury && cat /games/adventure/extra/opened.txt && echo "The door opens" > /dev/speaker || cat /games/adventure/extra/locked.txt
```

"If a file named `key` is in this room, create the treasury (copied
from the read-only cart), print the opened text, speak; otherwise print
the locked text." The treasury does not exist until the door is opened,
so the map genuinely grows.

**The last lever is broken on purpose.** `treasury/finish` is not
executable. Running it gives the machine's standard message, which names
the fix: `chmod +x finish`. The sign says the same. Then it prints the
badge, copies it to `~/badges/adventure.txt`, and tells the kid that a
bare `cd` goes home from anywhere.

## Changing it

Add a room: make a folder under `rooms/`, give it a `sign`, mention it
in the parent room's sign. Add a puzzle: a script with `&&`/`||`. Keep
every sign ending with the next command. If a room needs state, use a
file's existence, never a variable: files are what the kid can see.
