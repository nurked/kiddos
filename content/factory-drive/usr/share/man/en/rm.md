# rm
> remove a file (forever!)

## WHAT IT DOES
Deletes files. There is no trash can and no undo. Gone is gone.
That is the rule on real computers too, so learn it here where nothing
important can be lost.

## TRY THIS
```
touch junk.txt
rm junk.txt
mkdir box
rm -r box
```

## OPTIONS
- `-r` also remove folders and everything inside them
- `-f` do not complain if the file is not there

## SEE ALSO
rmdir, mv, cp

## GROWN-UP NOTE
`rm -rf /` does not work here; the machine's own folders belong to root.
The kid's home is fully theirs, including the right to wipe it.
