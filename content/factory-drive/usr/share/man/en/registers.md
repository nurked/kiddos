# registers
> the CPU's 31 boxes, the flags, and which branch is which

## WHAT IT DOES
Not a command: the page `debug` wants you to have read.

A CPU keeps numbers in registers. Here there are 31 of them, x0 to x30,
each holding a 64-bit number. The same boxes seen as 32-bit are w0 to
w30. Two more have jobs:
- `sp`, the stack pointer: the address of the top of the stack, the
  scratch space that grows downward from the top of memory.
- `pc`, the program counter: the address of the next instruction. You
  cannot write it with mov; branches change it.

By habit (the "calling convention", which real programs follow):
- x0-x7 carry arguments into a function and x0 carries the answer out
- x8 holds the system call number for `svc`
- x9-x15 may be scribbled on by any function you call
- x19-x28 are kept safe by functions (save them if you use them)
- x29 is the frame pointer, x30 (`lr`) is where `ret` goes back to
- `xzr` reads as zero and swallows writes

## THE FLAGS
`cmp a, b` subtracts b from a and throws the answer away, keeping only
four facts about it, the flags N Z C V. The branch instructions read
them:
- `b.eq` equal / `b.ne` not equal
- `b.lt` `b.le` `b.gt` `b.ge` less, less-or-equal, greater, greater-or-equal, for numbers that can be negative
- `b.lo` `b.ls` `b.hi` `b.hs` the same for numbers that are never negative (addresses, sizes)

The difference matters: -3 is less than 0 (`b.lt`), but as an unsigned
number -3 is enormous (`b.hi` would jump). The debugger's flags line
shows which branches would jump right now.

## SEE ALSO
asm, as, debug, syscalls

## GROWN-UP NOTE
This is the AArch64 procedure call standard, simplified. The emulator
does not enforce it: nothing stops a program from using x19 without
saving it, which is exactly the kind of bug bug-hunt is about.
