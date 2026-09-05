# goc
> compile a Go program: goc hello.go

## WHAT IT DOES
Turns a Go program into a `.wasm` file the machine can run. Go is the
language many big servers are written in. Here it talks only to the
`kiddos` package: the screen, keys, clock, sound, voice and your files.

## TRY THIS
```
cp /usr/share/examples/hello.go .
cat hello.go
goc hello.go
./hello.wasm
cat /usr/share/go/kiddos/kiddos.go
```

## OPTIONS
- `-o name.wasm` choose the output name
- `-v` show the compiler's own words instead of my translation

## SEE ALSO
cc, wasm, basic, gfx

## GROWN-UP NOTE
The compiler is TinyGo with a Go toolchain and wasm-opt, installed by a
parent as the "Go pack". Programs import "kiddos" (a package on the
drive); there is no fmt, os or net. `goc` adds an exported entry that
calls your main, because TinyGo's bare wasm target does not.
