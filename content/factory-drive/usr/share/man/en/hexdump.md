# hexdump
> show a file's bytes as numbers

## WHAT IT DOES
Every file is bytes: numbers from 0 to 255. hexdump shows them sixteen
per row, in hexadecimal (base 16: 0-9 then a-f), with the letters they
stand for on the right. Try it on a text file, then on a program.

## TRY THIS
```
echo Hello > hi.txt
hexdump hi.txt
hexdump -n 32 /bin/hello
hexdump /games/paint/paint.bas | head
```

## OPTIONS
- `-n 64` stop after 64 bytes

## SEE ALSO
dis, cat, wc, asm

## GROWN-UP NOTE
Same layout as `hexdump -C`: offset, 8 + 8 bytes, `|text|`. Works on
standard input too (`cat file | hexdump`).
