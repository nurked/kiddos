# share
> pack a game folder into a .kdc to give away: share /home/kid/rocket

## WHAT IT DOES
Parent mode only. Zips a game folder (one with a cart.toml, like the
ones `newgame` makes) into a `.kdc` file in the cartridge folder on the
real computer. Copy that file to another KidDOS and `install` it there.
A `.kdc` is an ordinary zip file; you can look inside it.

## TRY THIS
```
parent
share /home/kid/rocket
share /games/snake
carts
```

## SEE ALSO
install, carts, newgame
