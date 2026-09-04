# Architecture notes (Phases 0–2)

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
`crt off` draws it flat.

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

## Decisions taken on the plan's open questions

1. Name: still "KidDOS" (crate prefix `kiddos`).
2. Grid: 80x25.
3. `rm` in the kid's home is real and forever; trash semantics deferred.
4. BASIC: EndBASIC 0.12, pinned for its Apache license (see above).
5. `vi` ships locked (not registered at all yet).
6. Monetization: not addressed.
7. Languages: English only. The string layer and content directories are
   per-language so more can be added later; none is planned until asked.
