# arm
> run an assembled program (or just ./prog)

## WHAT IT DOES
Runs a program made by `as`. You do not usually type it: `./hello` does
the same, because the machine sees what kind of file it is.

## TRY THIS
```
as hello.s
arm hello
./hello
```

## SEE ALSO
as, asm, debug, wasm

## GROWN-UP NOTE
The interpreter the kernel picks for files that start with `\0arm`, the
way `\0asm` files go to `wasm`. Ctrl-C stops any loop; a fault prints
one sentence and the line it came from.
