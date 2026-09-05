# dis
> show a program's instructions: dis hello

## WHAT IT DOES
Reads an assembled program back into text: each instruction's address,
its four bytes, and what they mean, with the source line it came from
next to it. Then the data: every byte, with the letters on the right.

This is what the machine sees. Compare it with `cat hello.s`.

## TRY THIS
```
cp /usr/share/examples/hello.s .
as hello.s
dis hello
hexdump hello
```

## SEE ALSO
as, asm, debug, hexdump

## GROWN-UP NOTE
A real disassembler for the subset. `dis hello.s` assembles first. Words
the decoder does not know come out as `.word 0x...`; the literal pool
shows up as `.quad` lines after the code.
