# typing — Typing Practice

"Speed is the gate to everything else" (plan §9.3). The game measures
words per minute with `TICK`, and its ten lines are shell commands the
kid has already met, so it drills two things at once.

## Timing with TICK

```basic
start = TICK
... ten lines ...
ms = TICK - start
IF ms < 1 THEN ms = 1
wpm = 10 * 60000 / ms
```

`TICK` is milliseconds since boot. Integer division is fine here; the
guard keeps a zero out of the divisor (it happens in the test harness,
whose clock is virtual).

## Reading a line one key at a time

`INPUT` would work, but the game wants each keystroke, to echo it and to
allow ESC out at any time:

```basic
typed$ = ""
DO
    k$ = KEY$
    IF k$ = "ESC" THEN END 1
    IF k$ = "ENTER" THEN EXIT DO
    IF k$ = "SPACE" THEN k$ = " "
    IF LEN(k$) = 1 THEN
        typed$ = typed$ + k$
        PRINT k$;
    END IF
LOOP
```

`KEY$` names the space bar `SPACE`, so it is mapped back to a space
before comparing. The trailing `;` on `PRINT` keeps the cursor on the
line. There is no backspace: BASIC's console strips control characters,
so erasing on screen is not possible from here, and for a typing drill a
mistake simply counts.

## Reward

A correct line gets a two-note chime (`BEEP 660, 50` then
`BEEP 990, 80`) and a green "ok"; a wrong line is silent and shows what
it should have been in red. The result is spoken at the end. `END 1` on
ESC, `END 0` on completion, like every cartridge.
