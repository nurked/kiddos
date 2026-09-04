# chmod
> change what a file allows (like +x to run it)

## WHAT IT DOES
Every file has three permissions: r (read), w (write), x (run).
`chmod +x file` makes a script runnable. `chmod -w file` protects it from
changes. `ls -l` shows the permissions as letters like `-rwxr-xr-x`.

## TRY THIS
```
echo 'echo hi there' > greet
chmod +x greet
./greet
ls -l greet
```

## SEE ALSO
ls, ksh
