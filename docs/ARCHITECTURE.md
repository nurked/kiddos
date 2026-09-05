# Architecture notes (Phases 0–4)

This records how the plan in `kiddos-plan.md` was realized and where it was
bent. Read the plan first.

## Layers and the sandbox boundary

```
app            winit event loop, drive persistence, host-request handling
kiddos-host     RealHost: HostCaps impl (speech, beep, clock, config, password, log)
kiddos-render   Screen → RGBA texture → CRT shader
kiddos-kernel   Kernel, Proc (implements Console), Fs (jailed VFS view), pipes
kiddos-shell / kiddos-builtins / kiddos-man      programs; see only kernel + console + vfs
kiddos-console / kiddos-vfs / kiddos-i18n        leaf crates, no host access
```

`kiddos-builtins`, `kiddos-shell` and `kiddos-man` depend on `kiddos-kernel`,
which depends on `kiddos-console`, `kiddos-vfs`, `kiddos-i18n`. None of them can
name `kiddos-host` or `kiddos-render`. The one door to the real machine is the
`HostCaps` trait in `kiddos-kernel/src/host.rs`; `NullHost` (tests) and
`RealHost` (app) implement it.

## Processes are OS threads, not green threads

The plan says "green-thread processes with a cooperative scheduler". Phase 0
uses one OS thread per process instead:

* Every blocking console call (`readkey`, `sleep`, pipe reads/writes) polls a
  per-process `killed` flag every 50 ms, so Ctrl-C and `kill` work without a
  scheduler.
* Pipes are bounded byte queues with back-pressure; a writer whose reader has
  gone away is killed (our SIGPIPE), so `yes | head` terminates.
* Panics inside a command are caught; the kid sees "The program crashed."

The console API and the `Proc` type are what programs see, so swapping the
threading model later (or adding wasmtime epoch interruption in Phase 3) does
not change any program. The plan's intent — nothing can hang the machine —
holds.

## The VFS

In-memory inode tree. Permissions are owner `rwx` + other `rwx` (group bits
are stored and shown, treated as "other"). `root` bypasses everything; the
kid's user is `kid`. The machine's directories (`/etc`, `/games`, `/usr`,
`/bin`) are root-owned, so `rm /etc/motd` teaches "Permission denied" and
`~` is fully the kid's.

Persistence is a single SQLite file written whole via temp-file + rename
(atomic on every OS), debounced 500 ms in the app. The factory image is
built by `app/build.rs` from `content/factory-drive` and embedded in the
binary; `reset-drive` deletes the drive file and reboots.

At every boot the app refreshes the machine's own folders (`/etc`, `/usr`,
`/lessons`, `/dev`, and each game the factory ships under `/games`) from
the embedded image, so a drive made by an older build gets new games,
lessons and man pages. `/home` and games a parent installed are never
touched.

`/bin` is real: the kernel mirrors the command registry into it at boot so
`ls /bin` works. `/dev/null`, `/dev/tty`, `/dev/speaker` are intercepted in
the per-process `Fs` layer before the VFS sees them.

## The shell

`ksh` parses `;`, `&&`, `||`, `|`, `>`, `>>`, `<`, `2>`, single/double
quotes, `\`, `$VAR`/`${VAR}`/`$?`/`$1`, `~`, globs (`*`, `?`, `[..]`) and
`#` comments. `cd`, `exit`, `export`, `unset` and `history` run inside the
shell. Everything else is spawned. `NAME=value` alone sets a variable.

Unknown commands get a sentence and a Levenshtein suggestion. Scripts run by
shebang (`#!/bin/ksh`); a file without `+x` gets a message that names the
exact `chmod` to type.

The line editor supports cursor keys, Home/End, Ctrl-A/E/U/K/W/L, history
(Up/Down, `!!`, `!n`, `!prefix`, saved to `~/.ksh_history`), and Tab
completion for commands and paths (second Tab lists candidates).

## Rendering and fonts

There is no bitmap DOS font in the repo (nothing was downloaded). The shipped
font is the public-domain 8x8 set from the `font8x8` crate plus a hand-drawn
Cyrillic block in `kiddos-console/src/cyrillic.rs`, doubled vertically into
8x16 cells — the Amstrad CPC look. Glyphs are shared with `figlet`. A VGA
8x16 font can be dropped in later behind the `font` command.

The CPU rasterizes the 80x25 grid into a 640x400 RGBA texture only when the
screen generation or cursor blink changes; the WGSL shader letterboxes it to
4:3 and applies curvature, scan lines, glow, vignette and rounded corners.
`crt off` draws it flat. In pixel mode (below) the same texture is filled
from the 320x200 canvas at 2x instead; the shader does not know.

## Strings

`kiddos-i18n` parses a subset of Fluent syntax (`key = value`, indented
continuation lines, `{ $var }`) with zero dependencies. Files stay valid
`.ftl`, so the real `fluent` crate can replace the parser when plurals are
needed. Only English ships; other languages are a later phase and mean
adding a `.ftl` file plus content directories, not code.

## Kiosk (honest status)

Phase 0 does borderless fullscreen, hidden cursor, no macOS menu bar (so
Cmd-Q does nothing), and ignores window close while fullscreen. The macOS
presentation options (hide Dock, block Cmd-Tab) and the Windows keyboard hook
are Phase 4 work as planned.

## The tutor

`kiddos-tutor` subscribes to kernel events. After every shell line it gets
`CommandRun { line, status, cwd }`, matches the line against the current
step's glob patterns (`cd *`, `echo * > note.txt`), optionally checks the
machine (`cwd /games`, `file story.txt`, `contains greet.sh echo`), and
talks straight onto the screen in its own color before the next prompt.
Three misses earn a hint; `hint` asks for one. Lessons are TOML files in
`/lessons/en`; progress is `~/.progress`, badges are `~/badges/*.txt`. A
kid may edit `.progress` by hand: the tutor re-reads it.

Commands find the tutor through `Kernel::extension::<Tutor>()`, a typed
registry that keeps subsystems out of globals (one process can host many
machines, which the tests rely on).

Lessons use TOML rather than the YAML the plan suggests: it is already in
the stack, `serde_yaml` is unmaintained, and multi-line strings are enough.

## Cartridges

A cartridge is `/games/<name>/cart.toml` plus an entry the kernel can run
by shebang. `play` sets `CART` in the environment and grants only the
capabilities the manifest lists. The first cartridge, `adventure`, is pure
shell: it copies its room tree into `~/cave` so the kid can move and delete
things, and its levers are `ksh` scripts using `&&`/`||` and `2>` for the
locked-door logic. Unlocks (`vi` after vi-quest) wait for Phase 4.

### Sharing

A `.kdc` is a plain zip of a cartridge folder. It travels through one host
directory, `carts/` beside the drive file, reached only through three
`HostCaps` methods (list, read, write), so the "four files" rule becomes
"four files and one folder". Parent mode: `share <folder>` packs,
`install <name>` unpacks into `/games/<name>` as root (executable bits and
shebang files become 755), `uninstall` removes, `carts` lists. Kids get
`newgame <name>`, which scaffolds a runnable BASIC cartridge in `~`.
`export` could not be the command name: it is the shell's variable builtin.

## `edit`

A nano-alike in `kiddos-builtins/src/edit.rs`, full-screen through the
console API. Commands marked `keep_alive` receive Ctrl-C as a key instead
of being killed, so the editor can ask before discarding changes.

## BASIC

`kiddos-basic` embeds EndBASIC **0.12.0, pinned**: 0.13 and later relicensed
to AGPL-3.0, which would pull the whole app under AGPL; 0.12 is Apache-2.0
and has no networking. The interpreter runs inside the `basic` process:

* `KidConsole` implements EndBASIC's `Console` over `Proc` (so `PRINT`,
  `INPUT`, `CLS`, `COLOR`, `LOCATE`, `INKEY` all hit the KidDOS screen and
  key queue). Colors use CGA/QBasic numbering (14 = yellow).
* `KidDrive` implements its `Drive` over the VFS, mounted as `HOME:` so
  `SAVE "x"` writes `~/x.bas` and `cat x.bas` works from the shell.
* The KidDOS statements `SPEAK`, `BEEP`, `KEY$`, `TICK`, `PUT` are
  `Callable`s that map 1:1 onto the console API.
* Ctrl-C reaches a busy program through EndBASIC's yield hook (called
  between instructions): it takes a queued Ctrl-C and sends `Signal::Break`,
  so `WHILE TRUE: WEND` stops. `basic` is `keep_alive`, so the REPL survives
  a Ctrl-C and only `EXIT`/`QUIT`/Ctrl-D leave it.
* The REPL loop is ours (EndBASIC's treats `EXIT` as a loop keyword and
  `END` as "leave"); errors get a one-line hint.
* `run x.bas` and a `#!/bin/basic` shebang both work; a shebang line is
  stripped before compiling.

EndBASIC 0.12 quirks the cartridges work around: `MID` is 0-indexed (later
versions changed it), `STR` adds a leading space (`LTRIM` it), `READ` needs
a plain variable (not `arr(i)`), a variable must be assigned before its
first read in source order, and two sibling `FOR` loops nested inside
another `FOR` make its compiler panic on duplicate exit labels, so such
code lives in a `GOSUB` subroutine instead.

## The WASM sandbox

`kiddos-wasm` embeds wasmtime (40.x, the newest this Rust supports) with
cranelift and nothing else: no threads, no GC, and no WASI beyond the
subset in `wasi.rs` that maps onto the drive (Phase 5). A module gets one
import module, `kiddos`, whose functions are the console API one to one
(`print`, `put`, `getkey`, `readkey`, `readline`, `sleep`, `tick`,
`beep`, `speak`, `random`, `exit`, `fs_read`, `fs_write`, ...). Keys are
integers: a code point for printable keys, `0x110000 + n` for named
keys, `0x120000 + letter` for Ctrl. `/usr/include/kiddos.h` on the
drive mirrors all of it and is the entire C API: there is no libc.

Limits: 16 MB of memory (`StoreLimits`), one instance, and epoch
interruption. A ticker thread bumps the engine epoch every 10 ms; the
deadline callback checks for a queued Ctrl-C or a kill and traps with
`Interrupt`, so a `while (1)` loop stops like anything else. Traps and
link errors are turned into one sentence ("The program reached outside
its memory. (In C that is usually an array index...)").

The kernel recognises the `\0asm` magic and runs such files through the
`wasm` command, so `./hello.wasm` just works and cartridges can ship a
`.wasm` entry.

`cc` never runs a compiler inside the machine. Source files and
`kiddos.h` go out through one `HostCaps` method and a `.wasm` comes back;
the host runs a real clang (`packs/c/bin/clang` beside the drive, or
`KIDDOS_CC`) with `--target=wasm32 -nostdlib -Wl,--no-entry
-Wl,--export-all` in a scratch folder it deletes afterwards (clang names a
no-argument `main` `__main_void`; the runtime accepts both). Diagnostics
are rewritten as "hello.c, line 3: expected ';'... Every statement ends
with a semicolon."; `cc -v` shows clang's own words. Without the pack,
`cc` says so and points at docs/PACKS.md.

Packs are `.kdp` zips installed by parent mode (`install-pack`) into
`packs/<name>/` through three more `HostCaps` methods; `tools/mkpack.sh`
slices a 36 MB C pack out of a wasi-sdk release (clang, wasm-ld and the
two LLVM dylibs they reference, found by following `otool -L`).

`goc` works the same way with TinyGo (`packs/go/bin/tinygo`, plus a
bundled GOROOT and `wasm-opt`): the kid's `.go` files, the `kiddos`
package from `/usr/share/go/kiddos` and a generated entry file exporting
`kiddos_main` go out; TinyGo's bare `wasm-unknown` target never calls
Go's `main`, so the runtime calls `_initialize` then `kiddos_main`. See
docs/PACKS.md, including why there is no Pascal yet.

The proof program from the plan is `rogue`, a roguelike in C shipped as
source plus a 9 KB `.wasm`; see docs/cartridges/rogue.md.

## vi, locked commands, and the two vi games

`kiddos-vi` holds a modal editing engine (`engine.rs`: normal, insert,
command and search modes; hjkl, w/b/e, 0/$/^, gg/G, counts, x/X/D, dd,
dw, yy/yw, p/P, r, J, ~, u with an undo stack, `/` with n/N, `:w :q :q!
:wq :N`, ZZ). It knows nothing about screens or files: keys in, events
out. `vi` wraps it with a file and the E37/E32/E492 messages; vi-quest
wraps it with rules (allowed keys, stone, goals); Prison Escape wraps it
with doors.

The kernel gained *locked commands*: `register_locked` keeps a command
out of the registry until `unlock(name)`, which also appends the name to
`~/.unlocks`; at boot a locked command whose name is in that file is
registered straight away. The shell answers a locked name with "vi is
locked. You earn it by finishing a game", not "I don't know vi". This is
the plan's "unlock tools by learning them", and `vi` is its first use.

Cartridges may name a built-in command as their `entry`; the folder then
carries only docs and levels. Levels are TOML on the drive.

## Pixel mode (Phase 5)

`kiddos-console/src/pixels.rs` is a 320x200 canvas of palette indices,
double-buffered, with a 256-entry palette (0-15 CGA, 16-31 grays, 32-247
a 6x6x6 cube, 8 spares) and the primitives: pixel, line (Bresenham),
rect, fill, circle (midpoint), blit with a transparent color, read, text
with the 8x8 font, flip. `Screen` owns an `Option<Box<Pixels>>`: entering
pixel mode allocates it and the text cells stay intact underneath, so
leaving is a `take()` and the shell's screen is back. Only `flip` and
palette changes bump the screen generation; drawing to the back buffer
is invisible and free of redraws.

The `Console` trait is API v2: the v1 methods are untouched and the
`gfx_*` methods plus `key_held`/`key_event` were added. Every drawing
call enters pixel mode on its own (`Proc::ensure_gfx`), so a C program's
first `kd_gfx_pixel` just works; `gfx_mode(false)` or the process ending
brings the text back (`Proc::close` checks a `gfx_owner` flag, so Ctrl-C
and crashes restore the screen too).

Key state lives in the kernel: the host reports presses and releases
(`push_key_event`), the kernel keeps a held set and a capped event queue.
Releases also clear the case-swapped character, since a key pressed with
Shift may be released without it, and the window losing focus releases
everything. Entering pixel mode clears the event queue so a game does
not replay the shell's typing. Presses still go through the old key
queue, so `readkey` and the shell are unchanged.

BASIC: `KidConsole` implements EndBASIC's `size_pixels` and `draw_*`
methods, which gives the stock `GFX_PIXEL/LINE/RECT/RECTF/CIRCLE/CIRCLEF`
and `GFX_SYNC` for free, drawing in the current `COLOR`. Added: `SCREEN
13`/`SCREEN 0` (the QBasic spelling kids' books use), `PALETTE`,
`GFX_TEXT`, `GFX_FLIP`, `GFX_GET`, `KEYDOWN("LEFT")`, and the file words
`READFILE`, `WRITEFILE`, `APPENDFILE` that the C and Go bindings already
had. Rule: a program that ends in pixel mode keeps its picture up until
a key is pressed (`finish_gfx`), so a first `GFX_CIRCLE` at the prompt
does not vanish; after a Break or an error the text returns at once so
the message is seen.

WASM: the `kiddos` import module gained `gfx_*`, `key_down`, `key_event`;
`gfx_blit` reads the sprite from module memory (capped at 4096x4096),
`key_event` packs the key code with bit 24 set for a release. Mirrored in
`/usr/include/kiddos.h` and the Go package.

The paint cartridge is BASIC (so a kid can read it) and saves pictures as
36 lines of 64 characters, `.` and `A`-`O`, which `cat` and `edit`
understand.

### WASI, and Doom

Doom needs a libc. Writing a freestanding one (malloc, printf, fopen) was
the plan; building against wasi-libc and giving the sandbox a small
`wasi_snapshot_preview1` was less code and far fewer bugs, so that is
what `kiddos-wasm/src/wasi.rs` is. It maps stdout and stderr onto the
process's streams, stdin onto `readline`, the clock onto `tick`, `exit`
onto the same `Exit` error the `kiddos` module uses, and files onto the
virtual drive under the process's user: one preopened directory `/`,
whole files held in memory while open and written back on close or
sync. Networking, directory listing and everything else answer
`ENOSYS`. A program linked with wasi-libc therefore works when it only
uses the console API, when it only uses stdio, or both; the sandbox
story is unchanged, since nothing here can name a host path.

The Doom cartridge (`carts/doom/`) is doomgeneric with a 130-line
platform file: `CMAP256` makes Doom render into an 8-bit 320x200 buffer
with its own palette, so a frame is one `gfx_blit` and one `gfx_flip`,
and the palette is re-uploaded only when Doom changes it. Keys come from
`key_event`, run is always on (a permanently pressed Shift), and saves
go to `~/.doom` through the WASI file calls. `build.sh` needs a full
wasi-sdk, clones doomgeneric at a pinned commit, fetches Freedoom 0.13
and zips a `.kdc`. The manifest says `memory_mb = 64`; `play` passes
that as `KIDDOS_MEMORY_MB`, which the sandbox reads and caps at 256.

Doom exposed a kernel bug: Ctrl-C during `play <game>` killed the
interruptible `play` process and swallowed the key, while the BASIC or
WASM child, which watches the key queue for Ctrl-C itself, never saw
it. Processes now declare `handle_ctrl_c(true)` and the kernel queues
the key as well whenever such a process is alive.

## Decisions taken on the plan's open questions

1. Name: still "KidDOS" (crate prefix `kiddos`).
2. Grid: 80x25.
3. `rm` in the kid's home is real and forever; trash semantics deferred.
4. BASIC: EndBASIC 0.12, pinned for its Apache license (see above).
5. `vi` ships locked, and is unlocked by finishing vi-quest.
6. Monetization: not addressed.
7. Languages: English only. The string layer and content directories are
   per-language so more can be added later; none is planned until asked.
