# KidDOS

A fantasy computer for learning to drive a real one.

A single native app opens fullscreen and presents a retro machine: a CRT
terminal, a Unix-flavored shell, a virtual hard drive, a manual, and (later)
BASIC, a WASM sandbox for C/Go/Pascal, and game cartridges. There is no
internet, no host filesystem, no mouse. A child types `hi` and the machine
talks back.

Status: **v0.5, end of Phase 4** (foundations, tutor, BASIC, the WASM
sandbox with C and Go, vi earned by playing). Next is Phase 5, graphics. A kid can boot, explore, make files, read the manual, follow twelve
lessons with a tutor that watches the shell, write with `edit`, program in
BASIC, play seven cartridges (a cave adventure whose rooms are folders, and
guess, snake, hangman, typing, tetris and sokoban in BASIC that the kid can
read, copy and change, plus a roguelike in C), share games as `.kdc` files,
and, with the C or Go pack installed by a parent, write C with `cc` or Go
with `goc` and run it in a sandbox. `vi` is on the machine but locked:
Prison Escape teaches `:q!`, and finishing vi-quest earns the editor. See [kiddos-plan.md](kiddos-plan.md) for the full plan,
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how it is built,
[docs/cartridges/](docs/cartridges/README.md) for the cartridge format and a
walkthrough of every shipped game, [docs/PACKS.md](docs/PACKS.md) for the C
toolchain pack, and [docs/KID-SESSION.md](docs/KID-SESSION.md) for what to
watch when a kid sits down.

## Run it

```bash
cargo run --release -p kiddos
```

Start windowed instead of fullscreen while developing:

```bash
KIDDOS_WINDOWED=1 cargo run -p kiddos
```

Keep the drive somewhere other than the default app-support directory:

```bash
KIDDOS_HOME=/tmp/kiddos-dev cargo run -p kiddos
```

The parent chord is **Ctrl+Alt+Shift+P** (Cmd works as Ctrl on macOS). Parent
mode can `exit-fullscreen`, `reset-drive`, `set-name`, `passwd`, read the `log`,
`shutdown`, and move games in and out: `carts`, `install`, `uninstall`, `share`.
Cartridges are `.kdc` files (plain zips) in the app's `carts/` folder next to the
drive, the only place files cross between the fake machine and the real one.
A kid starts their own with `newgame rocket`.

## Poke at it without a window

```bash
cargo run -p kiddos-headless -- -
```

Type commands; the 80x25 screen is printed after each one. Scripts of
keystrokes (with `{tab}`, `{up}`, `{ctrl-c}` tokens) run with
`cargo run -p kiddos-headless -- script.txt`; the same harness drives the
integration tests in `tools/headless/tests`.

## Test

```bash
cargo test --workspace
```

## Layout

```
crates/kiddos-console   cell grid, keys, the Console API contract
crates/kiddos-vfs       inode tree, Unix modes, SQLite image, factory import
crates/kiddos-kernel    processes, pipes, capabilities, host bridge
crates/kiddos-shell     ksh: lexer, parser, expansion, line editor, executor
crates/kiddos-builtins  every command, one file per group
crates/kiddos-man       Markdown man renderer, lookup, search, pager
crates/kiddos-tutor     lesson state machine (TOML lessons, ~/.progress, badges)
crates/kiddos-cart      cartridge manifest, launching, .kdc pack/unpack, install/share
crates/kiddos-basic     EndBASIC 0.12 bound to the console and drive; SPEAK, BEEP, KEY$, TICK, PUT
crates/kiddos-wasm      wasmtime sandbox: the `kiddos` import module, `wasm`, `cc`, `goc`
crates/kiddos-vi        the vi engine, `vi` (locked until earned), vi-quest, prison-escape
tools/mkpack.sh         wasi-sdk or LLVM → c-<os>-<arch>.kdp
tools/mkpack-go.sh      TinyGo + GOROOT + wasm-opt → go-<os>-<arch>.kdp
crates/kiddos-i18n      Fluent-syntax string bundles (English today; i18n-ready)
crates/kiddos-render    wgpu renderer with the CRT shader
crates/kiddos-host      window, keys, speech, sound, config dir, parent password
app/                   the binary (embeds the factory drive at build time)
content/factory-drive  what the kid sees on first boot: /etc, /home/kid, man pages, lessons, games
tools/headless         no-window machine + test harness
tools/mkdrive          content dir → drive.kdd
```
