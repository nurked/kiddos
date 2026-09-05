# bug-hunt
> eight tiny ARM programs, one bug each; find them with debug

## WHAT IT DOES
Copies eight broken assembly programs to `~/bug-hunt`, runs each one,
and tells you which ones print the right thing. None of them does, at
first. Fix them one by one; `play bug-hunt` checks again.

## TRY THIS
```
play bug-hunt
cat ~/bug-hunt/01-hello.s
debug ~/bug-hunt/01-hello.s
edit ~/bug-hunt/01-hello.s
play bug-hunt
```

## OPTIONS
- `play bug-hunt reset` puts the original broken programs back

## SEE ALSO
debug, as, dis, syscalls, registers

## GROWN-UP NOTE
The programs are plain text under `/games/bug-hunt/programs`. Each has
one planted bug of a classic kind: off-by-one, wrong register, missing
`ret`, read-before-write, wrong branch condition, infinite loop. The
checker assembles the kid's copy, runs it in the emulator with a step
limit, and compares its output to the expected text.
