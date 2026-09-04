# vi-quest

The plan's progression hook: `vi` is not on the machine until the kid
earns it, and this is how. It is the "Vim Adventures" idea done with
the real thing: each land is a text buffer, the cursor is the hero, and
the only way to move is vi's own keys.

## How it is built

The game is Rust (`crates/kiddos-vi/src/quest.rs`) on top of the vi
engine (`engine.rs`), the same engine the `vi` command uses. The
cartridge folder has no program; `cart.toml` names the command:

```
/games/vi-quest/
├── cart.toml            entry = "vi-quest"   (a command, not a file)
├── levels/01-hjkl.toml  ... 10-wq.toml
├── README.md
└── man/vi-quest.md
```

`play` runs a command when the entry is not a file in the folder. That
is how a Rust game still gets a folder, docs and a manual page.

## Levels are data

```toml
title = "The land of hjkl"
story = """..."""            # shown above the map
keys = ["h", "j", "k", "l"]  # the only spells this land knows; "any" for all
goal = "reach"               # reach: stand on X; text: buffer == target; quit: :wq
hint = "..."                 # shown after 25 keys
done = "..."                 # said when the land is won
map = """
############
#@         #     @ is where you start, # is stone, X is the exit
#      X   #
############"""
```

Three rules make a vi buffer into a game:

- **Spells**: in normal mode, a key not in `keys` is refused with the
  list of what is allowed. Inside insert, command or search mode every
  key is allowed (you have to be able to type `/X` or `mat`).
- **Stone**: after every key, if the cursor ended up on `#`, the engine
  state is rolled back and the game says "Stone". This is what turns
  `hjkl` into a maze. Nothing else blocks; `x` removes a boulder `o`
  precisely because it is an ordinary letter.
- **Goals**: `reach` checks the character under the cursor, `text`
  compares the whole buffer to `target`, `quit` waits for `:wq`.

The ten lands: hjkl, w/b, 0/$, gg/G, x, dd, yy/p, /, i/a/Esc, :wq.

## The unlock

Winning the last land calls the kernel's `unlock("vi")`: the command
moves from the locked registry to the live one, `/bin/vi` appears, and
the name is appended to `~/.unlocks` (a file the kid can `cat`), which
the kernel reads at boot. A badge lands in `~/badges/vi-quest.txt`.

## Making a land

Add `levels/11-something.toml`. Keep maps under 70 columns and 15 rows
(the story band takes the top of the screen). Make sure the goal is
reachable with only the listed keys; the integration test plays every
land with a fixed key sequence, so a new land needs one too.
