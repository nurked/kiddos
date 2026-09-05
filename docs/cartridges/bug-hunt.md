# bug-hunt

`play bug-hunt` copies eight tiny ARM assembly programs to `~/bug-hunt`,
assembles and runs each one with no screen, and compares what it printed
with what it should have printed. Every program has exactly one bug. The
kid reads the file, watches it in `debug`, fixes it in `edit`, and plays
again to see the line turn green. All eight: a badge.

## The folder

```
/games/bug-hunt/
├── cart.toml          entry = "bug-hunt" (a command, like vi-quest)
├── README.md
├── man/bug-hunt.md
└── programs/
    ├── 01-hello.s     wrong length: prints "Hello" instead of "Hello!\n"
    ├── 02-add.s       wrong register: add x0, x1, x1
    ├── 03-count.s     off by one: b.lt where b.le was meant
    ├── 04-ret.s       a function with no ret falls into the next one
    ├── 05-order.s     reads the box before writing 7 into it
    ├── 06-loop.s      sub instead of add: the loop never ends
    ├── 07-strlen.s    ldr (8 bytes) where ldrb (1 byte) was meant
    └── 08-sign.s      b.hi (unsigned) on -3, so it looks positive
```

## How the checker works

The command lives in `crates/kiddos-arm/src/bughunt.rs`. For each
puzzle it reads the kid's copy, assembles it (an assembler error is
reported as such), runs it in the emulator with a `Quiet` I/O that
collects output and answers reads with end-of-input, stops after
200,000 instructions ("never finishes"), and prints one line per
program: `OK`, `BUG expected ... got ...`, `BUG crashes: ...`, or `ERR
does not assemble`. `play bug-hunt hint` prints the hint for the first
unsolved one; `play bug-hunt reset` copies the originals back.

The headless test `bug_hunt_reports_each_bug_and_rewards_all_eight`
applies the intended fix to each file in turn and checks that exactly
that program turns green, so a change to a puzzle that removes its bug,
or adds a second one, fails the build.

## Why these eight

They are the mistakes every programmer makes in every language, in the
form assembly shows most plainly: a count that is one short, the wrong
name, a function that does not come back, a value used before it is
set, a loop that never ends, reading more than you meant, and comparing
a negative number as if it could not be negative. Each one is found by
looking at a register or a memory byte in `debug`, not by guessing.
