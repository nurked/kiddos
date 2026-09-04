#!/bin/sh
# mkpack.sh: build a KidDOS C toolchain pack (.kdp) from a wasi-sdk or LLVM.
#
#   tools/mkpack.sh <wasi-sdk-or-llvm-dir> <out-dir>
#
# A pack is a zip with pack.toml, bin/clang and bin/wasm-ld, and whatever
# else the compiler needs to turn C into wasm32 with -nostdlib. Nothing
# else: no sysroot, no libc, because KidDOS programs only see kiddos.h.
# The result is named c-<os>-<arch>.kdp. A parent drops it into the
# carts/ folder and runs install-pack c in parent mode.
set -e
SRC="$1"; OUT="${2:-.}"
[ -d "$SRC/bin" ] || { echo "usage: $0 <toolchain-dir> [out-dir]"; exit 2; }
OS=$(uname -s | tr '[:upper:]' '[:lower:]'); ARCH=$(uname -m)
case "$OS" in darwin) OS=macos;; esac
case "$ARCH" in aarch64) ARCH=arm64;; amd64) ARCH=x86_64;; esac
WORK=$(mktemp -d); PACK="$WORK/c"; mkdir -p "$PACK/bin"
# clang and wasm-ld are often symlinks (clang -> clang-19, wasm-ld -> lld);
# copy the real files under the names clang expects.
cp -L "$SRC/bin/clang" "$PACK/bin/clang"
if [ -e "$SRC/bin/wasm-ld" ]; then cp -L "$SRC/bin/wasm-ld" "$PACK/bin/wasm-ld"; else cp -L "$(command -v wasm-ld)" "$PACK/bin/wasm-ld"; fi
chmod 755 "$PACK/bin/"*
# shared libraries the two binaries need (wasi-sdk's clang is a thin
# executable over libclang-cpp and libLLVM). Follow @rpath references
# transitively on macOS; on Linux take the whole lib/ folder.
if [ "$OS" = macos ]; then
  mkdir -p "$PACK/lib"
  queue="$PACK/bin/clang $PACK/bin/wasm-ld"
  while [ -n "$queue" ]; do
    next=""
    for f in $queue; do
      for dep in $(otool -L "$f" | awk '/@rpath|@loader_path|@executable_path/ {print $1}'); do
        base=$(basename "$dep")
        [ -e "$PACK/lib/$base" ] && continue
        src=$(find "$SRC/lib" -maxdepth 1 -name "$base" | head -1)
        [ -n "$src" ] || { echo "missing library $base"; exit 1; }
        cp -L "$src" "$PACK/lib/$base"; next="$next $PACK/lib/$base"
      done
    done
    queue="$next"
  done
elif [ -d "$SRC/lib" ]; then
  mkdir -p "$PACK/lib"; cp -L "$SRC"/lib/*.so* "$PACK/lib/" 2>/dev/null || true
fi
VER=$("$PACK/bin/clang" --version | head -1)
cat > "$PACK/pack.toml" <<TOML
name = "c"
language = "C"
description = "C compiler for KidDOS: $VER"
os = "$OS"
arch = "$ARCH"
TOML
# prove it works before packing
cat > "$WORK/t.c" <<C
int main(void) { return 42; }
C
( cd "$WORK" && PATH="$PACK/bin:$PATH" "$PACK/bin/clang" --target=wasm32 -O2 -nostdlib -Wl,--no-entry -Wl,--export-all t.c -o t.wasm ) || { echo "the sliced toolchain cannot build wasm32; not packing"; exit 1; }
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd); FILE="$OUT/c-$OS-$ARCH.kdp"; rm -f "$FILE"
( cd "$WORK" && zip -q -r -9 "$FILE" c )
rm -rf "$WORK"
echo "wrote $FILE ($(du -h "$FILE" | cut -f1)) - $VER"
