# Watching a kid use it

Sit next to them. Don't help unless they ask twice. Note what happens.

## Run it

```bash
cargo build --release -p kiddos
./target/release/kiddos
```

Fullscreen, no menu bar. Parent chord: Ctrl+Alt+Shift+P (Cmd+Alt+Shift+P
also works on a Mac). First time, `parent` asks you to choose a password.
To leave: `parent`, then `exit-fullscreen` (the window can then be closed)
or `shutdown`.

To start fresh for the next kid: `parent`, `reset-drive`, `yes`.

Drive, config, log and cartridges live in
`~/Library/Application Support/KidDOS/` (Mac). `log.txt` records lessons
finished, games played, parent-mode entries: read it after the session.

## What to watch for

- **The first minute.** Do they type `hi`? Do they press Enter without
  being told? Do they read the tutor's purple line or skip it?
- **Where they get stuck.** Which lesson step, and did the hint after three
  misses help or annoy? (`progress` shows the step; `cat ~/.progress` too.)
- **Typos.** Does "Did you mean ls?" get used, or do they retype from
  scratch? Do they discover Tab and the Up arrow on their own?
- **Reading.** Is the CRT look fine or do they lean in? `crt off` is one
  command; try both halfway through and ask which they prefer.
- **The screen filling up.** Do they find `clear`? Does `help` paging
  (Space / q) make sense?
- **The adventure.** Do they carry the torch with `mv`, and does "the pit"
  teach `ls -a` without a grown-up? Where do they call for help?
- **BASIC.** After `basic`, do they try their own lines or only what the
  lesson says? Does `edit` feel like the editor they expect?
- **Games.** Which one first, how long, do they look at the source
  (`cat /games/snake/snake.bas`) when told it is a text file?
- **vi.** Prison Escape: do the guard's hints land, or does the kid give
  up before the chalk? vi-quest: which land stops them?
- **Sound and voice.** Does `speak` delight or embarrass? Is the typing
  chime rewarding?
- **Leaving.** Do they ever try to quit? What do they press?

## After

```
parent
log
```

Then write down the three things that went worst. Those are the next
three tasks.
