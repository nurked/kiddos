#!/bin/sh
# Runs inside the container: build three targets, package into /src/dist.
set -e
cd /src
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
JOBS=${JOBS:-4}
build() { # target pkgconfig-libdir
  echo "=== $1"
  PKG_CONFIG_LIBDIR="$2" cargo build --release -p kiddos --target "$1" -j "$JOBS"
}
build x86_64-unknown-linux-gnu /usr/lib/x86_64-linux-gnu/pkgconfig
build aarch64-unknown-linux-gnu /usr/lib/aarch64-linux-gnu/pkgconfig
build x86_64-pc-windows-gnu /nonexistent
mkdir -p dist
for t in x86_64 aarch64; do
  D=dist/linux-$t; rm -rf "$D"; mkdir -p "$D"
  cp "target/$t-unknown-linux-gnu/release/kiddos" "$D/kiddos"
  cp docs/PARENTS.md "$D/READ-ME-FIRST-PARENTS.md"
  ( cd dist && tar czf "KidDOS-linux-$t.tar.gz" "linux-$t" )
done
D=dist/windows-x86_64; rm -rf "$D"; mkdir -p "$D"
cp target/x86_64-pc-windows-gnu/release/kiddos.exe "$D/kiddos.exe"
cp docs/PARENTS.md "$D/READ-ME-FIRST-PARENTS.md"
( cd dist && rm -f KidDOS-windows-x86_64.zip && zip -q -r KidDOS-windows-x86_64.zip windows-x86_64 )
ls -la dist/*.tar.gz dist/*.zip
echo "built v$VERSION"
