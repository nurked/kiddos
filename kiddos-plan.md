# KidDOS — Master Plan

Working title: **KidDOS** (rename later; "DOS" is branding, the vocabulary is Unix).
Owner: Ivan Roganov. Status: planning. Date: 2026-09-03.

---

## 1. One-paragraph definition

A single native application (Rust) that opens fullscreen and presents a fake retro computer: a CRT-styled terminal, a Unix-flavored shell, a virtual hard drive, a built-in help/man system, a BASIC interpreter, a sandboxed WASM runtime for C/Go/Pascal, and a cartridge format for small terminal games. There is no internet, no host filesystem, no windows. A child sits down, types `hi`, and the machine talks back. Everything the child touches is simulated; the only bridge to the real OS is a whitelisted capability table (speech, clock, sound).

Not an emulator. Not a Linux distro. Not a browser app. A fantasy computer whose "GPU" is a text grid and whose "game" is learning to drive a computer.

---

## 2. Principles (decide once, stop arguing)

1. **Unix vocabulary, pure.** `ls`, `cd`, `cat`, `mkdir`, `rm`, `man`. Never `dir`, never `type`. Skills must transfer to a real terminal.
2. **Nothing is real.** No host FS access, no host processes, no network stack. The kid cannot break the family computer and cannot escape into it.
3. **The machine is a character.** It speaks in first person, short sentences, kid register. Tone is part of the spec, not a polish pass.
4. **Everything is a file.** Games, lessons, man pages, the tutor's state, high scores. If it's in the VFS, the kid can `ls` it, `cat` it, `cp` it, and learn from that.
5. **Cartridges, not plugins.** A game is a folder with a manifest. Adding a game never requires recompiling the app.
6. **Robust over iterative.** Core (VFS, shell, console API) is designed once and versioned. Content iterates freely.
7. **Parent is root.** Parent mode has a password, can exit fullscreen, reset the drive, install cartridges, see a log.

---

## 3. Target user and scope

- Primary: children ~7–12 who can read. Secondary: parents who remember DOS/BBS/C64 and want to hand that down.
- Platforms: macOS (Apple Silicon + Intel), Windows 10/11, Linux (x86_64, aarch64/Raspberry Pi later).
- Languages: English at launch. All strings externalized from day one so more can be added later (i18n is a later phase).
- Input: keyboard only. No mouse. Ever.
- Out of scope v1: multiplayer, cloud, accounts, real Linux. Graphics is Phase 5.

---

## 4. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  host layer (per-OS): window, fullscreen, key grab, TTS,     │
│  audio, clock, parent-exit, persistence path                 │
├──────────────────────────────────────────────────────────────┤
│  renderer: text grid → GPU quad + CRT shader (wgpu)          │
├──────────────────────────────────────────────────────────────┤
│  console: 80x25 (configurable) cell buffer, colors, cursor,  │
│  key queue, tick. THE ONLY API games/programs ever see.      │
├──────────────────────────────────────────────────────────────┤
│  kernel: VFS, process table, scheduler, capabilities,        │
│  env vars, pipes, exit codes                                 │
├───────────────┬──────────────┬───────────────┬───────────────┤
│  shell (ksh)  │  builtins    │  BASIC        │  WASM runtime │
│  parser, jobs │  ls cat ...  │  (embedded)   │  (wasmtime)   │
├───────────────┴──────────────┴───────────────┴───────────────┤
│  content: man pages, tutor scripts, lessons, cartridges,     │
│  locale bundles — all inside the VFS image                   │
└──────────────────────────────────────────────────────────────┘
```

Single process, single binary. Games and programs run as green-thread "processes" inside the kernel with a cooperative scheduler; WASM programs run on wasmtime with fuel metering so a `while(1)` cannot hang the machine.

### 4.1 Host layer
- Windowing: `winit` + `wgpu`. Fullscreen borderless, cursor hidden, keyboard grabbed.
- Kiosk hardening per OS (see §8).
- Capabilities exposed to kernel via a trait: `speak(text, lang)`, `beep(freq, ms)`, `play(sample)`, `now()`, `persist(read/write image)`. That trait is the entire surface area between fake and real.
- TTS backends: macOS `AVSpeechSynthesizer` (or shell out to `say`), Windows SAPI, Linux `espeak-ng` bundled. Behind a single `Speaker` trait; falls back to a beep + printed text if unavailable.

### 4.2 Renderer
- Cell grid → texture atlas of a bitmap font (IBM VGA 8x16 or Amstrad CPC font; ship both, `font` command switches).
- 16-color CGA/EGA palette default; 256-color optional for games that ask.
- CRT shader: curvature, scanlines, phosphor glow, slight bloom. Toggleable (`crt off`) — some kids will hate it, and accessibility matters.
- Pixel mode (Phase 5): a 320×200, 256-color, double-buffered framebuffer, exclusive with text mode, uploaded in place of the text raster through the same shader.
- Target 60 fps, but the console is retained-mode: only redraw on dirty cells + shader pass.

### 4.3 Console API (the contract)
This is the ABI everything programs against. Freeze it early, version it.

```
console.size() -> (cols, rows)
console.put(x, y, ch, fg, bg)
console.print(str)           // at cursor, handles \n and scroll
console.cursor(x, y) / cursor_show(bool)
console.clear(bg)
console.getkey() -> Option<Key>   // non-blocking
console.readkey() -> Key          // blocking (yields)
console.readline(prompt) -> String
console.sleep(ms)                 // yields
console.tick() -> u64             // ms since boot
console.beep(freq, ms)
console.speak(text)               // capability-gated
fs.*                              // VFS ops, path-jailed to process cwd/home
```

Exposed identically to BASIC (as statements), to WASM (as host imports), and to Rust builtins. One API, three bindings. Games written against it are portable across all three.

### 4.4 Kernel
- **VFS**: in-memory tree with inodes (dir/file/symlink), owner, mode bits (simplified rwx so `chmod` is teachable), mtime. Backed by a single image file on disk (`drive.kdd`), written atomically on change with debounce. Format: SQLite (dead simple, crash-safe, inspectable) — not tar.
- **Layout** (kid sees this):
  ```
  /
  ├── bin/          (virtual; `ls /bin` lists builtins — teaches "where do commands live")
  ├── home/kid/     (their world; starts with a welcome letter and a `games/` symlink)
  ├── games/        (installed cartridges, read-only)
  ├── lessons/      (tutor content, read-only)
  ├── usr/share/man/
  ├── etc/          (motd, hostname, kiddos-release — all cat-able)
  ├── tmp/
  └── dev/          (null, tty, speaker — `echo hello > /dev/speaker` talks. This is the hook.)
  ```
- **Processes**: PID table, parent/child, exit codes, `$?`. Scheduler is cooperative on yields (readkey/sleep/print) plus WASM fuel preemption. `ps`, `kill`, `Ctrl-C` all work — these are teachable moments.
- **Capabilities**: each process has a cap set (`speak`, `sound`, `fs:/home/kid`, `fs:/tmp`). Cartridge manifests request caps; parent mode approves. Builtins get what they need.
- **Pipes and redirection**: `|`, `>`, `>>`, `<`. Real, because `ls | grep game > list.txt` is the single most magical thing a kid can learn.

### 4.5 Shell (`ksh` — "kid shell")
- POSIX-ish subset: command word, args, quoting, `$VAR`, `~`, globbing, `;`, `&&`, `||`, pipes, redirects, `#` comments.
- No `$(...)`, no heredocs, no functions in v1. Scripts (`.sh`) with shebang supported so kids can write `#!/bin/ksh`.
- Line editing: arrows, history (`history`, `!!`, up-arrow), tab completion for paths and commands. Tab completion is non-negotiable; it's how you discover.
- Unknown command → "I don't know `foo`. Try `help` or `man -k foo`." Never a raw error code.
- Typo tolerance: Levenshtein suggest ("Did you mean `ls`?").

### 4.6 Builtins (v1 command set)
Navigation/files: `ls`, `cd`, `pwd`, `cat`, `less`, `head`, `tail`, `mkdir`, `rmdir`, `rm`, `cp`, `mv`, `touch`, `tree`, `find`, `du`, `df`
Text: `echo`, `grep`, `wc`, `sort`, `uniq`, `rev`, `tr`, `cut`, `fortune`, `cowsay`, `figlet`
System: `help`, `hi`, `man`, `apropos`, `whoami`, `hostname`, `date`, `cal`, `uptime`, `ps`, `kill`, `sleep`, `clear`, `history`, `env`, `export`, `exit`
Learning: `tutor`, `lesson`, `hint`, `progress`, `badges`
Programs: `edit` (nano-like), `basic`, `run`, `cc` (C→wasm, phase 3), `games`, `play <name>`
Machine: `speak`, `beep`, `crt`, `font`, `lang`, `reboot`, `shutdown`, `parent`

`vi` is intentionally absent from v1. It arrives as a *game* (see §10) that, once completed, installs `/bin/vi`. Unlocking tools by learning them is the core progression mechanic.

### 4.7 Help and man system
- `man <cmd>` renders a real man page (NAME/SYNOPSIS/DESCRIPTION/EXAMPLES/SEE ALSO) written in kid language, with a mandatory EXAMPLES section first-class.
- `help` is the friendly front door; `man` is the grown-up door. Both exist so the kid learns the real word.
- Man pages are Markdown in `/usr/share/man/<lang>/<cmd>.md`, rendered by a tiny formatter. Localized by directory.
- `man -k` / `apropos` searches descriptions.

### 4.8 The Tutor
- A state machine living in `/lessons/`, driven by a small script format (YAML): trigger → message → expected command pattern → hint chain → reward.
- Watches the shell: after each command it can react ("You made a folder! Now go inside it with `cd`.").
- Progression tracked in `/home/kid/.progress` (visible, cat-able, editable — cheating is exploration).
- Lesson arc (v1): boot & `hi` → `help` → `ls`/`cd`/`pwd` → files with `cat`/`echo >` → `mkdir`/`mv`/`cp`/`rm` → `man` → pipes → `edit` → shell scripts → BASIC → games unlock.
- Badges printed as ASCII art. Kids will `cat ~/badges/*`.

### 4.9 BASIC
- Embed **EndBASIC** core as a Rust crate (it's designed to be embedded). Bind its console/FS traits to KidDOS console/VFS. Keep its `EDIT`, `RUN`, `LIST`, `SAVE`, `LOAD` REPL feel.
- Fallback if EndBASIC integration fights us: `my_basic` via FFI (two C files).
- `basic` enters the REPL; `run foo.bas` executes; shebang `#!/bin/basic` works from ksh.
- Extend with KidDOS-specific statements: `SPEAK`, `KEY$`, `PUT`, `TICK`, matching the console API 1:1.

### 4.10 WASM runtime (phase 3)
- `wasmtime` embedded, fuel-metered, memory-capped (16 MB default), no WASI except a KidDOS-specific import module `kiddos` exposing the console API.
- Any language that targets wasm32 becomes a "pass-through interpreter" for free:
  - C: bundle a `clang` wasm32 toolchain slice (or `wasi-sdk`); `cc hello.c` produces `hello.wasm`. Alternatively ship `tcc` and compile on the host to wasm — evaluate size.
  - Go: TinyGo → wasm. Toolchain is ~100 MB; make it an optional downloadable pack installed via parent mode.
  - Pascal: Free Pascal has a wasm32 target. Same treatment.
- Compile errors are translated into kid-readable messages by a post-processor (line number + plain-language hint). Raw compiler output is available with `-v` for the brave.
- Cartridges can ship prebuilt `.wasm` so toolchains are only needed for kids who write in C/Go/Pascal.

### 4.11 Cartridge format
```
games/snake/
├── cart.toml        # name, version, author, entry, caps, min_kiddos, lang, difficulty, unlocks
├── snake.bas        # or main.wasm, or main.sh
├── README.md        # shown by `play snake --about` and cat-able
├── man/snake.md     # auto-mounted into man
└── assets/          # text files, ASCII art, sound samples (wav ≤ 100 KB)
```
- Installed by dropping into `/games` via parent mode (or bundled in the drive image).
- `unlocks` field lets a cartridge grant a builtin or lesson on completion (e.g. vim-game unlocks `/bin/vi`).
- Signed by KidDOS keys optional; unsigned carts are allowed but flagged in parent mode. No internet means no store; distribution is a `.kdc` zip a parent copies in.

### 4.12 Parent mode
- `parent` command → password prompt → parent shell with extra commands: `exit-fullscreen`, `reset-drive`, `install <cart>`, `caps`, `log`, `set-lang`, `set-name`, `passwd`.
- Also reachable via a hardware chord (e.g. Ctrl+Alt+Shift+P) so it works even if the shell is wedged.
- Password stored hashed (argon2) in the host config dir, not in the VFS.

### 4.13 Persistence
- `~/Library/Application Support/KidDOS/` (mac), `%APPDATA%\KidDOS\` (win), `~/.local/share/kiddos/` (linux):
  - `drive.kdd` (SQLite VFS image), `config.toml`, `parent.hash`, `log.txt`
- Autosave on every VFS mutation (debounced 500 ms), snapshot on clean shutdown, `reset-drive` restores factory image shipped inside the binary.

---

## 5. Repo layout

```
kiddos/
├── Cargo.toml                 # workspace
├── crates/
│   ├── kiddos-console/         # cell grid, keys, API trait
│   ├── kiddos-vfs/             # inodes, SQLite backing, path resolution
│   ├── kiddos-kernel/          # processes, scheduler, caps, pipes
│   ├── kiddos-shell/           # ksh parser + line editor
│   ├── kiddos-builtins/        # every command, one file each
│   ├── kiddos-man/             # markdown man renderer, index
│   ├── kiddos-tutor/           # lesson state machine
│   ├── kiddos-basic/           # EndBASIC bindings
│   ├── kiddos-wasm/            # wasmtime host, kiddos import module
│   ├── kiddos-cart/            # cartridge manifest, install, unlocks
│   ├── kiddos-vi/              # modal editor engine, vi-quest, prison-escape
│   ├── kiddos-arm/             # AArch64 subset VM, assembler, debugger (Phase 6)
│   ├── kiddos-render/          # wgpu, font atlas, CRT shader
│   ├── kiddos-host/            # winit window, TTS, audio, kiosk, paths
│   └── kiddos-i18n/            # fluent bundles
├── app/                       # the binary
├── content/
│   ├── man/en/
│   ├── lessons/en/
│   ├── carts/
│   └── factory-drive/         # built into drive.kdd at build time
├── tools/
│   ├── mkdrive/               # content/ → drive.kdd
│   ├── cartpack/              # folder → .kdc
│   └── headless/              # run kiddos with no window for tests
└── docs/
```

Crate boundaries are the sandbox boundaries. `kiddos-builtins`, `kiddos-basic`, `kiddos-wasm` depend on `kiddos-console` and `kiddos-vfs` only — they cannot see `kiddos-host`. That's enforced by the dependency graph, not by discipline.

---

## 6. Key decisions and rationale

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Single static binary, wasmtime/EndBASIC/wgpu are all native Rust, memory safety in a kid sandbox |
| Rendering | wgpu + winit | Cross-platform, real fullscreen control, shaders for CRT; not Electron (leaks kiosk, 200 MB, mouse-first) |
| VFS backing | SQLite | Atomic, crash-safe, single file, inspectable by parent, trivial reset |
| Vocabulary | Unix | Transferable; kid can later open Terminal.app and feel at home |
| Sandbox for compiled langs | WASM | One sandbox for C/Go/Pascal; fuel + memory limits; no native code ever runs |
| BASIC | EndBASIC | Embeddable Rust, built by a parent for the same purpose, active |
| Games | Cartridge folders | No recompiles, kid-inspectable, community-portable |
| Progression | Unlock tools by learning | Turns curriculum into a game loop without gamifying with points |
| Content format | Markdown/YAML/TOML | Non-programmers (translators, teachers) can contribute |

---

## 7. Security / sandbox model

- Host FS: the app touches exactly four files under its config dir. Nothing else, ever. No file dialogs.
- Network: none compiled in. No `reqwest`, no sockets. Verify with `cargo deny` / dependency audit in CI.
- WASM: no WASI. Custom import module only. Fuel-metered; a runaway program gets "Your program is taking too long. Press Ctrl-C to stop it." Memory hard cap.
- BASIC: interpreter runs on the kernel scheduler; loop bound check every N statements yields.
- Caps: speech/sound rate-limited (no infinite `speak` loops screaming at 3 a.m.). Parent can revoke per cart.
- Content: cartridges are data. Nothing in a cart can reach the host layer.
- Parent password: argon2, lockout after 5 tries for 5 minutes.

---

## 8. Kiosk strategy per platform (honest limits)

- **macOS**: `NSWindow` fullscreen + `kiosk` presentation options (`NSApplicationPresentationOptions`: hide dock, hide menu bar, disable process switching, disable force quit, disable hide). Cmd-Tab/Mission Control are blockable *only with those options set*, which require the app to be the frontmost fullscreen app; Cmd-Q can be intercepted. Not 100 % — a determined 11-year-old with Activity Monitor knowledge wins. Good enough. Document "Guided Access-like" setup for parents who want hard lock (Screen Time app limits).
- **Windows**: Borderless fullscreen + low-level keyboard hook to swallow Win key, Alt-Tab, Alt-F4. Ctrl-Alt-Del is unblockable by design. Document Assigned Access for hard kiosk.
- **Linux**: Ship a `.desktop` session so it can be *the* session on a dedicated machine (Raspberry Pi + old monitor = ideal KidDOS box). In a normal DE, fullscreen + key grab.
- Rule: "hard to leave by accident, requires intent to leave on purpose." Parent chord always works.

---

## 9. Content plan

### 9.1 Man pages (v1: ~60)
One per builtin. Template enforced by CI: NAME, WHAT IT DOES (one sentence), TRY THIS (3 examples), OPTIONS, SEE ALSO, "GROWN-UP NOTE" (how this differs on real Linux/macOS).

### 9.2 Lessons (v1: 12)
1. Hello — `hi`, `help`, the prompt, Enter
2. Where am I — `pwd`, `ls`, `cd`, `..`
3. Files — `cat`, `echo`, `>`, `touch`
4. Building — `mkdir`, `mv`, `cp`, `rm` (with the "rm is forever" lesson and a `/tmp` sandbox)
5. Reading the manual — `man`, `apropos`, `--help`
6. The machine can talk — `/dev/speaker`, `speak`, `say`-style fun
7. Pipes — `|`, `grep`, `sort`, `wc`
8. Editing — `edit`
9. Scripts — `#!/bin/ksh`, `chmod +x`
10. Variables — `$NAME`, `export`, `env`
11. Your first program — BASIC `PRINT`, `INPUT`, `IF`, `FOR`
12. Make a game — BASIC `KEY$`, `PUT`, a loop → unlocks cartridge authoring lesson

### 9.3 Launch cartridges (v1: 8)
- `adventure` — Colossal-Cave-style text adventure whose map is the filesystem (Bashcrawl model): rooms are dirs, items are files, `cat` reads signs, `mv` picks things up. Teaches everything in §9.2 without saying so.
- `snake`, `tetris`, `sokoban` — console-API showcases in BASIC.
- `guess` — number guessing, first BASIC listing kids can `cat` and modify.
- `vi-quest` — Vim Adventures clone: grid world, move with hjkl, learn `dd`, `yy`, `/search` as spells. Completion unlocks `/bin/vi`. Written in Rust as a builtin cart (needs modal editing engine).
- `hangman` — vocabulary.
- `typing` — typing tutor, because speed is the gate to everything else.

### 9.4 Localization
- `fluent`-style bundles for UI strings; content dirs per language. English only for now; adding a language later is content (a bundle plus man/lesson dirs), not code.

---

## 10. Phases

### Phase 0 — Foundations (4–6 weeks)
- Workspace, CI (mac/win/linux builds, `cargo deny`), headless test harness.
- `kiddos-console`, `kiddos-vfs` (SQLite), `kiddos-kernel` (processes, pipes, caps).
- `kiddos-render`: fullscreen window, font atlas, CRT shader, 60 fps.
- `kiddos-shell`: parser, line editor, history, tab completion.
- 25 builtins. `hi`, `help`, `man` with 25 pages (EN).
- Parent mode: password, exit, reset.
- **Exit criterion**: a kid can boot, explore, make files, read man pages, get lost, get found. Ship as v0.1 to 5 families.

### Phase 1 — The Tutor (3–4 weeks)
- `kiddos-tutor` state machine, lessons 1–10, progress, badges.
- `edit`, scripts, variables, `/dev/speaker` with TTS on all three OSes.
- `adventure` cartridge (shell-only).
- **Exit**: v0.2. A kid with zero guidance reaches lesson 10 in a few sessions. Measure with the log.

### Phase 2 — BASIC (3 weeks)
- EndBASIC embedded, bound to console/VFS. `basic`, `run`, `edit` integration.
- Console-API extensions (`KEY$`, `PUT`, `TICK`, `SPEAK`).
- Cartridge format + installer. `snake`, `tetris`, `sokoban`, `guess`, `hangman`, `typing`.
- Lessons 11–12.
- **Exit**: v0.3. Kids modify `guess.bas` and share `.kdc` files with each other.

### Phase 3 — WASM & compiled languages (5–6 weeks)
- wasmtime host, `kiddos` import module, fuel/memory limits.
- `cc` via bundled wasm32 clang slice (measure size; target < 60 MB pack).
- Go (TinyGo) and Pascal (FPC) as optional parent-installed packs.
- Error message humanizer.
- Port one existing open-source terminal roguelike to a `.wasm` cart as proof.
- **Exit**: v0.4. `cc hello.c && ./hello` works on all three OSes.

### Phase 4 — vi-quest (done)
- Modal editor engine (`kiddos-vi`), `vi` registered locked.
- `prison-escape` cart (`:q`, `:q!`, `:wq`) and `vi-quest` cart (ten lands: hjkl, w/b, 0/$, gg/G, x, dd, yy/p, /, i/a/Esc, :wq); finishing unlocks `/bin/vi`, remembered in `~/.unlocks`.
- **Exit**: v0.5. A kid who has never seen vi leaves it, then earns it.

### Phase 5 — Graphics (done)
- **Pixel mode in the console**: 320×200 with a 256-color palette, double-buffered, exclusive with text mode (a program is in one or the other). 320×200 doubled is exactly the 640×400 text raster, so the renderer uploads the pixel buffer instead of the text raster and the CRT shader needs nothing. (320×240 is the 4:3 alternative if we ever drop the shared texture size.)
- **API in all three bindings** (console API v2, v1 kept intact): `pixel`, `line`, `rect`, `fill`, `blit`, `palette`, `text` (our 8×8 font drawn into the buffer), `flip`. Drawing goes to the back buffer; `flip` shows it.
- **Key down and key up events**, so games can hold a key. The host reports both; text mode keeps today's press-only `readkey`.
- **BASIC gets `GFX_*`** by implementing EndBASIC's five `draw_*` console methods (`draw_pixel`, `draw_line`, `draw_rect`, `draw_rect_filled`, `draw_circle`), which are no-ops today.
- **A paint cartridge** as the demo: brushes, colors, save to a file the kid can `cat`.
- **Then Doom**: `doomgeneric` compiled against wasi-libc (a real libc beat a hand-written one: the sandbox gained a small `wasi_snapshot_preview1` that maps stdio and files onto the virtual drive, ~600 lines, and every future C program gets `stdio.h` with it), a per-cartridge memory cap in `cart.toml` (`memory_mb = 64`), and Freedoom Phase 1 data. Shipped as `doom.kdc` (10.6 MB) for a parent to `install`, not in the factory drive, because the WAD alone is 28.8 MB.
- **Exit**: v0.6. Paint runs; Doom runs at playable speed on the Mac; both from cartridges. Done.

| Risk | Impact | Mitigation |
|---|---|---|
| Per-pixel host calls too slow from WASM/BASIC | Games stutter | Programs draw into their own memory and `blit` whole regions; `flip` is the only per-frame call |
| Doom's libc surface is large | Weeks of shims | Start from doomgeneric's short platform layer; stub what Freedoom never calls; measure before adding |
| Two modes to keep consistent | Bugs in every program | Exclusive modes: entering pixel mode saves the text screen, leaving restores it (the alt-screen path already exists) |

### Phase 6 — ARM assembly and debug tools (4–5 weeks)
Goal: a kid who can write BASIC and a little C gets to see the machine underneath: registers, memory, one instruction at a time. ARM because it is the CPU in the Mac, the Pi and every phone, and because AArch64 is regular enough to teach.

**Figure out first (one-week spike, decisions written into §6):**
- **Which ARM.** AArch64 user-mode subset: fixed 32-bit encodings, 31 general registers, no Thumb/ARM mode split. ~40 instructions cover teaching (`mov add sub mul udiv and orr eor lsl lsr cmp b b.cond bl ret ldr str ldrb strb adr svc`). Thumb/ARMv7 only if the subset turns out too big for kids, which I doubt.
- **Emulate, don't run native.** KidDOS ships on x86 Windows and Linux, and even on the Mac we never run kid code on the real CPU. A `kiddos-arm` crate with our own interpreter: deterministic, single-steppable, memory-capped, and every fault message is ours ("you read from address 0, which is nothing"). Unicorn/QEMU rejected: huge, C, licensing, and no kid-grade errors. Cost of writing our own: decoder for the subset plus tests against a reference (clang `-target aarch64` output run through our VM vs. expected results).
- **Assembler.** Our own two-pass `as` for the subset: labels, `.data`/`.text`, `.ascii`/`.byte`/`.word`, comments, friendly errors with the offending line. Output is a `\0arm` file the kernel resolves like `\0asm`, so `as hello.s && ./hello` mirrors `cc`.
- **System calls.** `svc #0` with the Linux AArch64 convention (`x8` = number, `x0..x5` = args) mapped onto the console API from §4.3: write, read key, read line, sleep, beep, exit, fs read/write. `man syscalls` lists them. Same numbers as real Linux where one exists (64 write, 93 exit) so what a kid learns here is true outside.
- **Debugger shape.** Not gdb's prompt. A full-screen `debug prog` with three panes: source with the current line highlighted, registers (changed ones flash), a memory window at a chosen address. Keys: step, run, breakpoint on the current line, continue, quit; a `:` line for `mem 0x1000`, `reg x0`, `break 12`. Reuse the vi engine's screen layout code, not gdb.
- **Debugging beyond assembly.** Decide whether `debug` also steps BASIC (EndBASIC has no hooks; would need our own line tracer) and C (DWARF in wasm is a large project). Default answer: assembly only in this phase; a `trace` flag for BASIC that prints each line as it runs is cheap and worth doing; C debugging goes to Later.

**Build:**
- `kiddos-arm`: VM (registers, flags, 1 MB flat memory, cap in `cart.toml`), decoder/executor for the subset, `svc` bridge, step/breakpoint API, disassembler.
- Commands: `as`, `debug`, `dis` (disassemble a file), `hexdump` (kid-readable bytes; useful for BASIC and C files too).
- Content: man pages `as`, `debug`, `dis`, `hexdump`, `syscalls`, `registers`; examples `/usr/share/examples/hello.s`, `count.s`, `echo.s`; lessons 13 "What a CPU does" (step through `add`) and 14 "Find the bug" (a wrong loop bound, fix it in `edit`, run again).
- Cartridge **bug-hunt**: eight tiny programs, each with one planted bug (off-by-one, wrong register, missing `ret`, reads before writes). The kid opens each in `debug`, finds it, fixes the source. Finishing unlocks nothing new; the reward is a badge and that `debug` now shows up in `games` hints.
- **Exit**: v0.7. `as hello.s && ./hello` prints; `debug` steps it with registers changing on screen; bug-hunt ships.

| Risk | Impact | Mitigation |
|---|---|---|
| Subset too small for anything fun | Kids leave after hello | Pick the subset from what bug-hunt and a small game (guess-the-number in asm) actually need, then freeze it |
| Encoding bugs in our own VM | Wrong answers teach wrong things | Differential tests: assemble with clang, run in our VM and in a known-good model, compare registers |
| Debugger UI is a second editor to maintain | Slow phase | Share the pane/scroll code with `kiddos-vi` and `edit`; the debugger is read-only, no editing inside it |

### Phase 7 — Polish and release (4 weeks)
The rest of what used to be Phase 4:
- Accessibility: `crt off` (done), large font, high-contrast palette, screen reader passthrough via TTS.
- Kiosk hardening per §8 (macOS presentation options, Windows keyboard hook, Linux session), Raspberry Pi session image.
- Installer/signing/notarization for mac + win.
- **Exit**: v1.0 public.

### Later
- Cartridge SDK docs + template repo. Community carts.
- Fake "modem" for LAN-only multiplayer BBS (no internet, but two KidDOS boxes on the same Wi-Fi is a *great* lesson).
- Hardware: dedicated KidDOS box (Pi + CRT-look monitor) — EndBOX proves the market exists.

---

## 11. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Kiosk escape on macOS | Parents lose trust | Set expectations in docs; parent chord; Screen Time guidance; never claim "unbreakable" |
| EndBASIC API churn / license fit | Phase 2 slip | Pin version, vendor fork, my_basic fallback |
| Toolchain bundle size (C/Go/Pascal) | Bloat, install failures | Optional packs; prebuilt `.wasm` in carts; C first, Go/Pascal later |
| Content is the product, and content is slow | v1 feels empty | Templates + checks; hire/recruit a teacher for lessons |
| Kids find it boring after `ls` | Retention | Adventure cart in Phase 1, not Phase 2; every lesson ends with something funny/loud |
| Scope creep toward "real Linux" | Never ships | Principle 2. Anything that needs a real kernel is a "Later" item |

---

## 12. Open questions (answer before Phase 0 starts)

1. Name. "KidDOS" implies DOS vocabulary. Alternatives: "KidNIX", "Tiny Terminal", "Prompt". Decide before the repo is public.
2. Grid size: 80×25 (authentic) vs 80×30/100×35 (more room for games). Recommend 80×25 default, carts may request larger.
3. Does the kid have `rm -rf ~` power? Recommend yes, with `trash` semantics under the hood and a `undo`/`restore` command in parent mode. The lesson matters.
4. EndBASIC vs writing own BASIC. Recommend EndBASIC; prototype the binding in week 2 of Phase 0 to confirm.
5. Ship `vi` locked (unlock via game) vs available from start. Recommend locked; it's the best progression hook in the design.
6. Monetization: free + paid cartridge packs? One-time purchase? Open-source core + paid content? Decide by end of Phase 1.

---

## 13. Stack summary

- Rust (stable), workspace of ~13 crates
- winit, wgpu, WGSL CRT shader
- rusqlite (bundled)
- EndBASIC (crate), my_basic (fallback, FFI)
- wasmtime (fuel, memory limits)
- fluent for i18n
- argon2 for parent password
- Platform TTS: AVSpeechSynthesizer / SAPI / espeak-ng
- Toolchains (optional packs): wasi-sdk clang, TinyGo, Free Pascal wasm32
- CI: GitHub Actions matrix, cargo deny, headless integration tests that replay keystroke scripts against the shell and diff the screen buffer
