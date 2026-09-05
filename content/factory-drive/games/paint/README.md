# Paint

A picture is 64 dots across and 36 dots down, and every dot is one of 16
colors. Move the white box with the arrows and press SPACE to paint a
dot. X wipes it. The numbers 0-9 and letters A-F pick a color from the
bar at the bottom.

S saves the picture as `picture.txt` in your home folder. It is only
text: `cat picture.txt` shows it, `edit picture.txt` lets you change it
with letters, and L brings it back into Paint.

The whole program is BASIC. `cat /games/paint/paint.bas` shows how it
draws: `GFX_RECTF` for a dot, `GFX_RECT` for the box, `GFX_TEXT` for the
help line, and `GFX_FLIP` once per key so nothing flickers.
