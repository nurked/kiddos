#!/bin/sh
# mkpack-go.sh: build a KidDOS Go toolchain pack (.kdp).
#
#   tools/mkpack-go.sh <tinygo-release-dir> <GOROOT> <wasm-opt> [out-dir]
#
# TinyGo needs three things at build time: itself (with its LLVM and its
# src/targets tree), a Go toolchain (GOROOT, for the standard library and
# the `go` command), and Binaryen's wasm-opt. The pack carries all three:
#
#   go/bin/tinygo        go/bin/wasm-opt        go/go/   (a GOROOT)
#   go/lib, go/src, go/targets, ...            (TinyGo's own tree)
#
# It is big (a few hundred MB). That is why it is a pack.
set -e
TG="$1"; GR="$2"; WO="$3"; OUT="${4:-.}"
[ -x "$TG/bin/tinygo" ] || { echo "usage: $0 <tinygo-dir> <GOROOT> <wasm-opt> [out-dir]"; exit 2; }
[ -x "$GR/bin/go" ] || { echo "$GR is not a GOROOT (no bin/go)"; exit 2; }
[ -x "$WO" ] || { echo "$WO is not wasm-opt"; exit 2; }
OS=$(uname -s | tr '[:upper:]' '[:lower:]'); ARCH=$(uname -m)
case "$OS" in darwin) OS=macos;; esac
case "$ARCH" in aarch64) ARCH=arm64;; amd64) ARCH=x86_64;; esac
WORK=$(mktemp -d); PACK="$WORK/go"
mkdir -p "$PACK"
# TinyGo's tree, minus things a kid's wasm build never touches (it does
# build a few wasm builtins from lib/wasi-libc and lib/musl on first use)
( cd "$TG" && tar cf - --exclude='./lib/picolibc' --exclude='./lib/mingw-w64' --exclude='./lib/nrfx' --exclude='./lib/cmsis' --exclude='./lib/macos-minimal-sdk' --exclude='./pkg' . ) | ( cd "$PACK" && tar xf - )
# a GOROOT: the go command, the standard library sources, the compiler's pkg
mkdir -p "$PACK/go"
( cd "$GR" && tar cf - --exclude='./test' --exclude='./doc' --exclude='./misc' --exclude='./api' --exclude='./pkg/tool/*/vet' --exclude='./pkg/tool/*/pprof' --exclude='./pkg/tool/*/trace' --exclude='./pkg/tool/*/doc' --exclude='./pkg/tool/*/cover' --exclude='./pkg/tool/*/nm' --exclude='./pkg/tool/*/objdump' . ) | ( cd "$PACK/go" && tar xf - )
# wasm-opt and whatever dylibs it needs
cp -L "$WO" "$PACK/bin/wasm-opt"; chmod 755 "$PACK/bin/wasm-opt"
if [ "$OS" = macos ]; then
  mkdir -p "$PACK/lib"
  for dep in $(otool -L "$PACK/bin/wasm-opt" | awk '/@rpath|@loader_path|@executable_path/ {print $1}'); do
    base=$(basename "$dep"); src=$(find "$(dirname "$WO")/../lib" -maxdepth 1 -name "$base" | head -1)
    [ -n "$src" ] && cp -L "$src" "$PACK/lib/$base"
  done
fi
VER=$("$PACK/bin/tinygo" version | head -1)
cat > "$PACK/pack.toml" <<TOML
name = "go"
language = "Go"
description = "Go compiler for KidDOS: $VER"
os = "$OS"
arch = "$ARCH"
TOML
# prove it works before packing, using only what is inside the pack
T="$WORK/t"; mkdir -p "$T/kiddos"
printf 'package main\n\nfunc main() {}\n\n//export kiddos_main\nfunc kiddosMain() { main() }\n' > "$T/main.go"
printf 'module kidprog\n\ngo 1.22\n' > "$T/go.mod"
( cd "$T" && env -i PATH="$PACK/bin:$PACK/go/bin:/usr/bin:/bin" HOME="$WORK" GOROOT="$PACK/go" GOFLAGS=-mod=mod GOPROXY=off "$PACK/bin/tinygo" build -target=wasm-unknown -opt=z -no-debug -o t.wasm . ) || { echo "the packed toolchain cannot build wasm; not packing"; exit 1; }
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd); FILE="$OUT/go-$OS-$ARCH.kdp"; rm -f "$FILE"
( cd "$WORK" && zip -q -r -9 "$FILE" go )
rm -rf "$WORK"
echo "wrote $FILE ($(du -h "$FILE" | cut -f1)) - $VER"
