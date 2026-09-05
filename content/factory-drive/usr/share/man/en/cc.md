# cc
> compile a C program: cc hello.c

## WHAT IT DOES
Turns a C program into a `.wasm` file the machine can run. C is the
language most of the world's computers are written in. Here it is safe:
a C program can only talk to the screen, the keys, the clock and your
files, like any other program.

## TRY THIS
```
cp /usr/share/examples/hello.c .
cat hello.c
cc hello.c
./hello.wasm
cat /usr/include/kiddos.h
```

## OPTIONS
- `-o name.wasm` choose the output name
- `-v` show the compiler's own words instead of my translation

## SEE ALSO
wasm, basic, edit, gfx

## GROWN-UP NOTE
There is no libc: `kiddos.h` is the whole API. The compiler is a real
clang with a wasm32 target, installed by a parent as the "C pack"; until
then `cc` says so. Programs run in wasmtime with a WASI subset that only
reaches the virtual drive, a 16 MB memory
cap, and Ctrl-C works even in a tight loop.
