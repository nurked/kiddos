# Bug Hunt

Eight tiny assembly programs. Each one is supposed to print something,
and each one has exactly one bug. Your job: find it and fix it.

`play bug-hunt` copies them to `~/bug-hunt` and checks them. Fix a
program with `edit` (or `vi`), then `play bug-hunt` again to see which
ones pass. `debug ~/bug-hunt/01-hello.s` lets you watch a program run
one instruction at a time: that is how you find the bug.

The bugs are the ones every programmer makes: a wrong number, a wrong
register, a loop that stops one too early (or never), a function that
forgets to come back, a value read before it was written.

All eight fixed: a badge in `~/badges`.
