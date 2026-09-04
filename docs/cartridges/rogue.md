# rogue

The proof that the WASM sandbox can host a real terminal game: a
roguelike in C, one file of about 280 lines, using nothing but
`kiddos.h`. It ships as a prebuilt `rogue.wasm` (9 KB) next to its
source, so it runs without the C pack, and a kid with the pack can
`cp /games/rogue/rogue.c ~/`, change it, and `cc rogue.c`.

## Files

```
/games/rogue/
├── cart.toml     entry = "rogue.wasm"
├── rogue.wasm    the compiled game (the kernel sees the \0asm magic)
├── rogue.c       the source, as the kid should read it
├── README.md
└── man/rogue.md
```

## How it is built

Rebuild the `.wasm` from the source with the same flags `cc` uses:

```
clang --target=wasm32 -O2 -nostdlib -fno-builtin -Wall -Wno-unused-function -I. \
      -Wl,--no-entry -Wl,--export-all -Wl,-z,stack-size=65536 rogue.c -o rogue.wasm
```

(`kiddos.h` must be in the include path; it is in
`content/factory-drive/usr/include/`.)

## No libc, on purpose

There is no `printf`, `rand`, `malloc` or `strlen`. The game keeps
everything in static arrays (`map[H][W]`, `monx[]`, `mony[]`), formats
its status line by hand, and gets randomness from `kd_random()`, which
is a host function. That is what makes a program safe here: the only
things it *can* do are the things in the header.

## Structure

- **`new_floor()`** places up to nine non-overlapping rooms, joins each
  to the previous one with an L-shaped corridor, drops the stairs in the
  last room, scatters gold and potions, and fills rooms with monsters
  chosen by depth.
- **`draw()`** paints the map with `kd_put`. Cells within a small
  rectangle of the player are lit; anything ever lit is remembered in
  `seen[][]` and drawn dim. The status line is rebuilt each turn.
- **The turn** is the classic loop: `kd_readkey()` waits, `player_move`
  handles walls, fights, gold, potions and stairs, then `monsters_move`
  gives every awake monster one step toward the player or one bite.
- **Fighting** is bump-to-attack: walking into a monster hits it. Damage
  scales with depth on both sides; potions heal fully; each floor down
  raises max HP.

## Changing it

Monster kinds are five parallel arrays at the top (`monchar`,
`monname`, `monmaxhp`, `mondmg`, `moncolor`); add a sixth and extend
`monster_for_depth`. The lit radius is the `8`/`5` in `draw`. Room
sizes are the `5 + rnd(10)` and `3 + rnd(4)` in `new_floor`.
