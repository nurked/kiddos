# find
> find files by name

## WHAT IT DOES
Walks through folders looking for files. Use `-name` with a pattern:
`*` means "anything".

## TRY THIS
```
find . -name '*.txt'
find / -name '*.md'
find ~ -type d
```

## OPTIONS
- `-name pattern` only names that match (use quotes around `*`)
- `-type f` only files, `-type d` only folders

## SEE ALSO
ls, tree, grep
