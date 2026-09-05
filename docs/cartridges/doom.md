# doom

Not written for KidDOS: ported to it. The point of this cartridge is
that a real, famous program runs on the machine unchanged, with about
130 lines of glue. It is also the only cartridge a parent installs
(`install doom` from `doom.kdc`), because the Freedoom data is 28 MB.

## What is in the folder

```
/games/doom/
├── cart.toml            # entry = "doom.wasm", memory_mb = 64
├── doom.wasm            # doomgeneric + wasi-libc, 660 KB
├── freedoom1.wad        # Freedoom Phase 1: levels, monsters, art
├── FREEDOOM-LICENSE.txt
├── README.md
└── man/doom.md
```

The sources are in the KidDOS repo under `carts/doom/`: a platform file,
a build script, and these docs. `build.sh <wasi-sdk>` fetches doomgeneric
at a pinned commit and Freedoom 0.13.0, compiles, and zips `dist/doom.kdc`.

## The platform layer

doomgeneric asks a port for six functions. Ours, in
`doomgeneric_kiddos.c`:

- `DG_Init`: `kd_gfx_mode(1)` and a permanently pressed Shift, so the
  kid always runs.
- `DG_DrawFrame`: Doom is built with `CMAP256`, so its screen buffer is
  already 320 x 200 bytes of palette indices, the machine's own format.
  One `kd_gfx_blit`, one `kd_gfx_flip`. Doom's palette (which changes
  when you are hit or pick something up) is uploaded with
  `kd_gfx_palette` only when it differs from the last one sent.
- `DG_GetKey`: fed from `kd_key_event`, which reports keys going down
  and up; Doom needs both to know how long you hold "forward". Arrows
  map to Doom's arrows, X to fire, Space to use, A and D to strafe.
- `DG_SleepMs`, `DG_GetTicksMs`: `kd_sleep`, `kd_tick`.
- `main`: makes `~/.doom`, changes into it (config and saves land
  there), and starts Doom with `-iwad $CART/freedoom1.wad`.

## The libc

Doom wants `fopen`, `fread`, `fseek`, `malloc`, `printf`, `mkdir`. Rather
than write those, the port links wasi-libc and the sandbox provides the
dozen WASI calls it makes (`fd_read`, `fd_seek`, `path_open`,
`clock_time_get`, `proc_exit`...), mapped onto the virtual drive and the
console. The WAD is read through `fd_read` in chunks like any file; the
config file is written through `fd_write` and lands on the drive on
close. Doom's startup log goes to stdout, which is the text screen under
the pixels: after quitting, the kid sees the same "W_Init: Init
WADfiles" lines a 1993 PC showed. See the WASI section of
[ARCHITECTURE.md](../ARCHITECTURE.md).

## Memory

Doom's zone allocator wants more than the sandbox's 16 MB. The manifest
says `memory_mb = 64`; `play` passes it down and the sandbox raises the
cap for that process only.

## Licenses

Engine GPL-2.0 (doomgeneric, from id Software's 1997 release), data
BSD-3-Clause (Freedoom). Both ship in the `.kdc`.
