# asm
> what assembly is, and the ARM this machine speaks

## WHAT IT DOES
Not a command: the story behind `as`, `debug` and `dis`.

Every program you have run here - BASIC, C, Go - was turned into tiny
steps before the CPU could do anything with it. Assembly is those steps
written out by hand. One line is one instruction. An instruction moves
a number, adds two numbers, compares, or jumps to another line. That is
all a CPU does, billions of times a second.

This machine speaks AArch64: the 64-bit ARM inside every phone, the
Raspberry Pi and the Mac. Here it runs in a small emulator, so it can be
stopped, stepped and watched.

A program looks like this (it is `/usr/share/examples/hello.s`):
```
.text
_start:
    mov x0, #1          // x0 = 1: the screen
    adr x1, msg         // x1 = where the text is
    mov x2, len         // x2 = how long it is
    mov x8, #64         // 64 means "write"
    svc #0              // ask the machine to do it
    mov x0, #0
    mov x8, #93         // 93 means "exit"
    svc #0
.data
msg: .ascii "Hello from ARM!\n"
len = . - msg
```

## TRY THIS
```
cp /usr/share/examples/hello.s .
as hello.s
./hello
debug hello
man registers
man syscalls
```

## THE WORDS
- **register**: one of 31 boxes in the CPU, x0 to x30, each holding a 64-bit number. w0 to w30 are their lower halves. `man registers`
- **instruction**: one line: `add x0, x1, x2` means x0 = x1 + x2. `man as` lists them all.
- **label**: a name for a place in the program (`loop:`), so you can jump there with `b loop`.
- **system call**: `svc #0` with a number in x8: the machine does something for you (print, read, exit). `man syscalls`
- **memory**: bytes with addresses. Your program starts at 0x10000; `.data` comes after it; the stack (`sp`) is at the top.

## SEE ALSO
as, debug, dis, registers, syscalls, hexdump, bug-hunt

## GROWN-UP NOTE
A real subset of AArch64 with the real encodings: what `as` produces is
what clang produces for the same line, and the machine is a
single-stepping interpreter (no native code ever runs). Linux's system
call numbers are used where Linux has one, so `mov x8, #64; svc #0` is
the same on a Raspberry Pi. What is missing: floating point, SIMD, most
of the 500-odd remaining instructions, and any notion of an operating
system beyond those calls.
