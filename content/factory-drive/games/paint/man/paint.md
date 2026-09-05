# paint
> draw pictures pixel by pixel in 16 colors, save them as text

## WHAT IT DOES
Paint is the first program that draws with dots instead of letters. The
screen becomes 320 x 200 pixels; Paint uses them as 64 x 36 big dots.

## KEYS
- arrows: move the white box
- SPACE: paint a dot in the current color, X: wipe it
- 0-9, A-F: pick a color (the bar at the bottom shows which)
- S: save as `~/picture.txt`, L: load it back, N: start over
- ESC: leave

## TRY THIS
```
play paint
cat picture.txt
edit picture.txt
```
A saved picture is text: `.` is empty and the letters A to O are the
colors 1 to 15. Change some letters in `edit` and load it again.

## SEE ALSO
gfx, basic, games
