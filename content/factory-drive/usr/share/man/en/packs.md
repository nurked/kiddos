# packs
> toolchain packs (compilers) on this machine

## WHAT IT DOES
Parent mode only. Compilers are big, so KidDOS ships without them. A
*pack* adds one: the C pack makes `cc` work, the Go pack makes `goc`
work. `packs` shows which are installed, which `.kdp` files are waiting
in the cartridge folder, and whether `cc` works right now.

## TRY THIS
```
parent
packs
install-pack c
```

## SEE ALSO
install-pack, remove-pack, cc, carts
