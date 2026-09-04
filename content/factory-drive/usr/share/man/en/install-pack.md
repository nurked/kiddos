# install-pack
> install a .kdp toolchain pack: install-pack c

## WHAT IT DOES
Parent mode only. Unpacks a `.kdp` file from the cartridge folder into
the packs folder on the real computer. Packs are built per operating
system: `c-macos-arm64.kdp` for a Mac with Apple silicon, and so on.
The docs explain where to get them (docs/PACKS.md).

## TRY THIS
```
parent
carts
install-pack c
exit
cc /usr/share/examples/hello.c -o hello.wasm
```

## SEE ALSO
packs, remove-pack, cc
