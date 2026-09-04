# ksh
> the kid shell (runs your scripts)

## WHAT IT DOES
The shell is the program you are talking to right now. It reads what you
type, finds the command, and runs it. It also understands a few special
things:

- `a ; b` run a, then b
- `a && b` run b only if a worked
- `a | b` send a's output into b (see man pipes)
- `a > file` send a's output into a file
- `$NAME` a variable
- `*.txt` every file ending in .txt
- `# words` a comment (ignored)

## SCRIPTS
Put commands in a file, one per line, starting with `#!/bin/ksh`. Make it
runnable with `chmod +x`. Now it is a program. Try `cat ~/bin/hello`.

## SEE ALSO
pipes, history, chmod, export
