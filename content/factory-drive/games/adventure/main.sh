#!/bin/ksh
# The Drive Below. Rooms are folders. Build the cave once, in the kid's home.
ls ~/cave > /dev/null 2> /dev/null || cp -r $CART/rooms ~/cave
cat $CART/intro.txt
echo "Welcome to the drive below" > /dev/speaker
