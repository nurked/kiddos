# pipes
> joining small commands into big ones

## WHAT IT DOES
The `|` sign (a pipe) takes what one command prints and hands it to the
next. Small tools, joined, do things no single tool can.

## TRY THIS
```
ls /bin | wc -l
ls /bin | grep s | sort -r
cat welcome.txt | tr a-z A-Z
fortune | cowsay
help | grep -i file > filestuff.txt
```

## THE ARROWS
- `>` send output into a file (replacing it)
- `>>` add output to the end of a file
- `<` read input from a file

## SEE ALSO
grep, sort, wc, tr, ksh
