# Rogue

A dungeon crawler in the oldest tradition: you are `@`, the dungeon is
letters, and every floor is made up fresh. Arrows move, walk into a
monster to fight, `p` drinks a potion, `>` goes down the stairs.

This one is written in **C**, not BASIC, and compiled to a `.wasm` that
the machine runs in its sandbox. The source is right here:

```
cat /games/rogue/rogue.c | less
cp /games/rogue/rogue.c ~/
cc rogue.c
./rogue.wasm
```

(`cc` needs the C pack, which a parent installs.)
