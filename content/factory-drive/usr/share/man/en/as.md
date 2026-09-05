# as
> assemble an ARM program: as hello.s

## WHAT IT DOES
Turns a text file of instructions into a program the machine can run.
`as hello.s` makes `hello`; `./hello` runs it. Every line is one
instruction, a label, or a note to the assembler (a line starting with
a dot). `man asm` is the introduction.

## TRY THIS
```
cp /usr/share/examples/count.s .
as count.s
./count
dis count
```

## OPTIONS
- `-o name` choose the output name

## THE INSTRUCTIONS
Registers are x0-x30 (64-bit) or w0-w30 (their 32-bit halves), plus sp
(the stack pointer) and xzr (always zero). `#5` is the number 5.

Moving numbers:
- `mov x0, #5` / `mov x0, x1` / `mvn x0, x1` (the bits flipped)
- `ldr x0, =0x123456789` for numbers too big for one mov

Arithmetic (the first register gets the answer):
- `add x0, x1, x2` / `add x0, x1, #5` / `sub` / `neg`
- `mul x0, x1, x2` / `udiv` / `sdiv` (u = never negative, s = signed)
- `and` `orr` `eor` `lsl` `lsr` `asr` for bits

Comparing and jumping:
- `cmp x0, x1` (or `cmp x0, #5`) sets the flags, then
- `b.eq label` `b.ne` `b.lt` `b.le` `b.gt` `b.ge` (signed)
  `b.lo` `b.ls` `b.hi` `b.hs` (unsigned) jump if the comparison says so
- `b label` always jumps; `cbz x0, label` jumps if x0 is zero (`cbnz`: not)
- `bl label` calls a function (remembers where to come back in x30); `ret` comes back

Memory:
- `ldr x0, [x1]` reads 8 bytes at the address in x1; `str` writes
- `ldrb w0, [x1]` / `strb` one byte; `ldrh`/`strh` two; `ldr w0`/`str w0` four
- `[x1, #8]` at x1 + 8; `[x1, x2]` at x1 + x2
- `str x0, [sp, #-16]!` pushes; `ldr x0, [sp], #16` pops
- `stp x29, x30, [sp, #-16]!` / `ldp` two at once (how functions save x30)
- `adr x1, label` puts a label's address in a register

Talking to the machine:
- `svc #0` with the call number in x8 (`man syscalls`)

Notes to the assembler:
- `.text` instructions follow; `.data` numbers and text follow; `.bss` empty space
- `msg: .ascii "Hi\n"` text; `.asciz` adds a 0 at the end
- `.byte 1, 2` / `.word 1000` (4 bytes) / `.quad 1` (8 bytes) / `.space 100`
- `len = . - msg` a name for a number; `.` is "here"
- `//` starts a comment

## SEE ALSO
asm, debug, dis, registers, syscalls

## GROWN-UP NOTE
GNU syntax, two passes, a literal pool for `ldr =`, real AArch64
encodings (checked against clang). Immediate ranges are the CPU's: 0-4095
for add/sub, 16 bits shifted for mov, bitmask patterns for and/orr/eor.
Errors name the line and suggest the nearest mnemonic.
