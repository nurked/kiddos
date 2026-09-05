# syscalls
> what a program can ask the machine to do: svc #0

## WHAT IT DOES
Not a command: the table every assembly program needs.

An assembly program cannot print by itself. It puts a number in x8, its
arguments in x0, x1, x2..., and runs `svc #0`. The machine does the job
and puts the answer in x0. The numbers below 1000 are the real Linux
numbers: the same program works on a Raspberry Pi.

## THE CALLS
```
 x8   name       x0, x1, x2...                          answer in x0
 63   read       0 (keyboard), buffer, size             bytes read
 64   write      1 (screen) or 2, text, length          bytes written
 93   exit       exit code                              never returns
101   nanosleep  address of [seconds, nanoseconds]      0
278   getrandom  buffer, size                           bytes filled

1000  readkey    -                                      the key (waits)
1001  getkey     -                                      the key, or -1
1002  sleep      milliseconds                           0
1003  beep       frequency, milliseconds                0
1004  tick       -                                      milliseconds since boot
1005  random     -                                      a random number
1006  readfile   path, path length, buffer, size        bytes read, or -1
1007  writefile  path, path length, data, length, append (1/0)   0, or -1
1008  put        column, row, character, color, background       0
1009  cursor     column, row                            0
1010  clear      background color                       0
1011  color      color, background                      0
1012  size       -                                      columns*65536 + rows
1013  speak      text, length                           1 if it can talk
```
Key numbers are the ones in `/usr/include/kiddos.h`: letters are
themselves, Enter is 0x110001, the arrows 0x110005-0x110008. A path
length of 0 means "up to the zero byte" (`.asciz`).

## TRY THIS
```
cp /usr/share/examples/echo.s .
as echo.s
./echo
```

## SEE ALSO
asm, as, registers, debug

## GROWN-UP NOTE
Linux AArch64 convention: number in x8, up to six arguments in x0-x5,
result in x0, `svc #0`. Only the calls listed exist; anything else is
"System call N does not exist". The 1000+ range is KidDOS-only and maps
onto the same console API C and Go use.
