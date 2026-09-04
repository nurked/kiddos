# Toolchain packs

Compiled languages need a compiler, and compilers are big. KidDOS ships
without one; a parent adds a *pack* by putting it in a folder next to the
drive file:

| OS | folder |
|---|---|
| macOS | `~/Library/Application Support/KidDOS/packs/` |
| Windows | `%APPDATA%\KidDOS\packs\` |
| Linux | `~/.local/share/kiddos/packs/` |

(`KIDDOS_HOME` overrides the base folder.)

## The C pack

`cc` looks for `packs/c/bin/clang` and expects it to be a clang that can
produce wasm32 with `wasm-ld` beside it. Two ways to get one:

- **wasi-sdk** (Bytecode Alliance releases): unpack it and point `packs/c`
  at it (a symlink is fine). It ships clang plus wasm-ld for all three OSes.
- **LLVM** from your package manager (`brew install llvm` on a Mac): link
  `packs/c` to the LLVM prefix, e.g. `ln -s "$(brew --prefix llvm)" packs/c`.
  Apple's own clang cannot: it has no `wasm-ld`.

For development, `KIDDOS_CC=/path/to/clang` overrides the pack.

Check it inside the machine:

```
cp /usr/share/examples/hello.c .
cc hello.c
./hello.wasm
```

`cc` compiles with `--target=wasm32 -O2 -nostdlib -fno-builtin`, links with
`--no-entry --export-all`, and includes only `/usr/include/kiddos.h`. No
libc, no WASI: a program can print, draw, read keys, sleep, beep, speak,
and read or write the kid's files. Nothing else exists.

## Go and Pascal

Planned as packs of the same shape (TinyGo and Free Pascal both target
wasm32); not wired up yet.
