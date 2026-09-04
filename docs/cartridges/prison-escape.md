# prison-escape

The game that teaches `:q!`. Everyone who has used a terminal has been
trapped in vi once; this turns that moment into three short cells.

## How it is built

Rust, in `crates/kiddos-vi/src/prison.rs`, on the same engine as `vi`.
Each cell is a `Cell` struct: a title, four story lines, the cell's
"walls" (a text buffer drawn with `#`), four timed hints, and whether
the buffer starts dirty. The kid's keys go straight into the engine, so
anything vi does, happens; only the door logic is the game's.

| cell | opens for | teaches |
|---|---|---|
| 1 | `:q` (or `:wq`) | Escape first, then a colon, then q, Enter |
| 2 | `:q!` only; `:q` shows E37, `:w`/`:wq` show E45 (read-only) | the bang |
| 3 | `:wq` after the buffer changed; `:q!` refused | write, then leave |

Hints arrive after 6, 12, 20 and 30 keys, each more explicit; the last
one is chalk on the wall with the exact command. Nobody stays trapped.

The messages are vi's own (`E37: No write since last change (add ! to
override)`), so the kid recognises them on a real machine.

## The ending

A badge in `~/badges/prison-escape.txt`, the number of keys it took, and
a pointer to vi-quest for the rest of the spells.
