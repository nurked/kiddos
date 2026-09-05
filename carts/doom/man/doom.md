# doom
> the 1993 game that made 3D happen, with Freedoom's free levels

## WHAT IT DOES
The real Doom engine, running in pixel mode. It draws 320 x 200 dots
thirty-five times a second and asks the machine for one blit and one
flip per frame, the same words as `man gfx`.

## KEYS
- arrows: turn and walk (you always run)
- A, D: step sideways
- X: shoot, SPACE: open doors and press switches
- Esc: menu, Enter: choose, Tab: map, 1-7: weapons
- F2 save, F3 load, F6 quicksave, F9 quickload

## TRY THIS
```
play doom
ls ~/.doom
```

## GROWN-UP NOTE
Engine: doomgeneric (GPL-2.0), data: Freedoom Phase 1 (BSD-3-Clause).
Built with `carts/doom/build.sh` in the KidDOS repo against wasi-libc; the
sandbox provides a WASI subset that maps files onto the virtual drive.
The cartridge asks for 64 MB of memory in its manifest. There is no sound
yet.
