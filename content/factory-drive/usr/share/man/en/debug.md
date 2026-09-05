# debug
> step through a program one instruction at a time

## WHAT IT DOES
Runs a program under a magnifying glass. The screen shows the source
with the next instruction lit, every register, a window into memory,
and what the program has printed. Press `s` and one instruction runs;
the registers that changed light up yellow and the bottom line says
what happened.

## TRY THIS
```
cp /usr/share/examples/count.s .
as count.s
debug count
```
Then press `s` a few times. Press `c` to let it run. Press `q` to leave.

## KEYS
- `s` (or Enter, or Space): one instruction
- `n`: one instruction, but a `bl` call counts as one (skip over the function)
- `c`: continue until a breakpoint, the end, or a crash. Ctrl-C stops a runaway.
- `b`: set (or remove) a breakpoint on the selected line; `c` stops there
- `r`: back to the start
- `.`: jump the view back to the current line; arrows move the selection
- `q`: quit
- `:` then a command:
  - `:mem msg` `:mem sp` `:mem x1+8` `:mem 0x10040` moves the memory window
  - `:break 12` `:break loop` a breakpoint by line or label; `:delete` clears all
  - `:reg x5` shows one register; `:goto 30` selects a line

## SEE ALSO
as, asm, dis, registers, syscalls, bug-hunt

## GROWN-UP NOTE
`debug prog` reads the source and line table `as` embedded in the
program; `debug prog.s` assembles on the spot. The debugger owns the
screen, so the program's output goes to its own pane, and a program that
reads a line gets asked for it on the status row. Registers show small
values in decimal and addresses as `0x10020 msg`. There is no
BASIC or C stepping yet: those interpreters have no line hooks.
