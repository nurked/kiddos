# tutor
> the tutor: what to do next (tutor off / on / list)

## WHAT IT DOES
The tutor watches what you type and pops up with a purple face when you
do something new. `tutor` tells you the current lesson and step.
Stuck? Type `hint`. Want quiet? `tutor off`. Curious? `tutor list`.

## TRY THIS
```
tutor
hint
progress
badges
lesson 3
```

## OPTIONS
- `tutor off` / `tutor on` silence or wake the tutor
- `tutor list` all lessons
- `tutor restart` start over from lesson 1
- `lesson N` jump to lesson N

## SEE ALSO
hint, progress, badges, help

## GROWN-UP NOTE
Lessons are TOML files in /lessons/en. Progress is in ~/.progress, a
plain file the kid may read and edit. Badges are ASCII art in ~/badges.
