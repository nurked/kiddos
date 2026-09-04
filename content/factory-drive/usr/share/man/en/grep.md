# grep
> find lines that contain a word

## WHAT IT DOES
Reads lines and shows only the ones that contain your word. It shines at
the end of a pipe: `something | grep word`.

## TRY THIS
```
grep folder welcome.txt
ls /bin | grep c
help | grep -i file
grep -n the welcome.txt
```

## OPTIONS
- `-i` ignore big/small letters
- `-n` show line numbers
- `-v` show lines that do NOT contain the word
- `-c` just count matching lines

## SEE ALSO
pipes, sort, wc, find

## GROWN-UP NOTE
Real grep uses regular expressions. Here a word is just a word, except
`^` (line start) and `$` (line end) work like in the real thing.
