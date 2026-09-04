# echo
> say something back (or put it in a file with >)

## WHAT IT DOES
Prints its words. Boring alone, magic with `>`: that sends the words into
a file instead of the screen.

## TRY THIS
```
echo hello
echo hello > note.txt
cat note.txt
echo more >> note.txt
echo -c red I am red
echo hello > /dev/speaker
```

## OPTIONS
- `-n` no new line at the end
- `-e` understand `\n` (new line) and `\t` (tab)
- `-c color` print in a color: red, green, yellow, blue, magenta, cyan, white

## SEE ALSO
cat, pipes, speak
