# vi
> the editor the grown-ups use (earned in vi-quest)

## WHAT IT DOES
vi is on every Linux and macOS computer on Earth, and on most servers.
It has no menus; it has modes. In NORMAL mode letters are commands:
h j k l move, x deletes, dd cuts a line, yy copies it, p pastes.
i enters INSERT mode, where letters are letters. Esc goes back.
: commands: :w saves, :q quits, :q! quits without saving, :wq both.

You cannot use vi until you finish `play vi-quest`. Then it is yours.

## TRY THIS
```
play prison-escape
play vi-quest
vi story.txt
```

## THE SPELLS
- `h j k l` left, down, up, right   `w b` next/previous word   `0 $` line start/end
- `gg G` top/bottom   `/word` find, `n` next   `:12` go to line 12
- `x` delete a letter   `dd` cut a line   `dw` cut a word   `yy` copy a line   `p` paste
- `i a o` insert here / after / on a new line   `Esc` back to normal   `u` undo
- `:w` save   `:q` quit   `:q!` quit anyway   `:wq` save and quit

## SEE ALSO
vi-quest, prison-escape, edit
