# Toolchain packs

Compiled languages need compilers, and compilers are big. KidDOS ships
without them; a parent adds a *pack*. A pack is a `.kdp` file (a plain
zip with a `pack.toml` at its root) built per operating system and CPU.
It goes into the cartridge folder and is installed from parent mode:

```
parent
carts            # shows go-macos-arm64.kdp waiting
install-pack c   # or install-pack go
packs            # what is installed, and whether cc works
```

Packs live next to the drive file:

| OS | folder |
|---|---|
| macOS | `~/Library/Application Support/KidDOS/packs/` |
| Windows | `%APPDATA%\KidDOS\packs\` |
| Linux | `~/.local/share/kiddos/packs/` |

(`KIDDOS_HOME` overrides the base folder.) Dropping an unpacked folder
there by hand works too; `packs/c/bin/clang` is all `cc` looks for.

## The C pack (`c-<os>-<arch>.kdp`, ~36 MB)

`bin/clang`, `bin/wasm-ld` and the two LLVM libraries they need, sliced
out of a [wasi-sdk](https://github.com/WebAssembly/wasi-sdk) release by
`tools/mkpack.sh`:

```bash
tools/mkpack.sh /path/to/wasi-sdk-34.0-arm64-macos packs/
```

The script checks that the slice can build wasm32 before zipping. Any
LLVM with a wasm32 target works as input (Homebrew's `llvm` plus `lld`,
for example); Apple's own clang does not, it has no `wasm-ld`.

`cc` compiles with `--target=wasm32 -O2 -nostdlib -fno-builtin`, links
with `--no-entry --export-all`, and includes only `/usr/include/kiddos.h`.
No libc, no WASI, no sysroot: a program can print, draw, read keys,
sleep, beep, speak, and read or write the kid's files. Nothing else
exists, so nothing else needs shipping.

## The Go pack (`go-<os>-<arch>.kdp`, ~200 MB)

TinyGo needs three things: itself (with its LLVM), a Go toolchain (the
`go` command and the standard library, as GOROOT), and Binaryen's
`wasm-opt`. `tools/mkpack-go.sh` bundles all three from a TinyGo release
tarball, a Go install and a wasm-opt binary, and test-builds a program
with only the pack's contents on PATH before zipping:

```bash
tools/mkpack-go.sh /path/to/tinygo "$(go env GOROOT)" /opt/homebrew/bin/wasm-opt packs/
```

`goc` writes the sources, the `kiddos` package from
`/usr/share/go/kiddos`, a `go.mod` with a `replace kiddos => ./kiddos`,
and a generated `zz_kiddos_entry.go` that exports `kiddos_main` calling
`main()` (TinyGo's `wasm-unknown` target never calls Go's `main` on its
own) into a scratch folder, then runs
`tinygo build -target=wasm-unknown -opt=z -no-debug`. The runtime calls
the module's `_initialize` and then `kiddos_main`.

## Pascal

Free Pascal 3.2.2, the current stable release, has no WebAssembly
target; the wasm32 backend is in the development branch (3.3.x) and is
expected in the next stable release. A Pascal pack will follow the same
shape when a release can produce wasm32 without a runtime we cannot
provide. Until then `pc` does not exist on the machine.

## For development

`KIDDOS_CC=/path/to/clang` and `KIDDOS_TINYGO=/path/to/tinygo` override
the packs. The test suite uses them (and `KIDDOS_TEST_KDP` /
`KIDDOS_TEST_GO_KDP` to exercise real packs); without them the compiler
tests pass vacuously and say so.
