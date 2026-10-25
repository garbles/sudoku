#!/bin/sh
# Makes manual-pen-rules-landscape.png in $1: page 4 of the manual in landscape (the text of manual-pen-rules.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=1.25; CW=175

text 72 170 84 black start "Pen and touch"

text 72 290 50 black start "Taps"
text 72 355 38 gray25 start "A tap is a pen touch that stays in one"
text 72 405 38 gray25 start "micro-cell. If the pen moves out of the"
text 72 455 38 gray25 start "micro-cell, the touch is writing, not a tap."
text 72 505 38 gray25 start "Finger touches on the board have no"
text 72 555 38 gray25 start "effect. Use the pen."
cell 200 600 "" 1
cell 550 600 "" ""
open_cell 550 600
echo "fill none stroke black stroke-width 3 circle 23,23 31,23 stroke-linecap round path 'M 23,23 L 72,74 M 60,72 L 72,74 L 70,62'" >> $M
echo "pop graphic-context" >> $M
text 287 825 34 gray25 middle "Tap"
text 637 825 34 gray25 middle "Not a tap"

text 72 920 50 black start "Not a digit"
text 72 985 38 gray25 start "If you write a mark that is not a number"
text 72 1035 38 gray25 start "from 1 to 9, the app shows ? for one"
text 72 1085 38 gray25 start "second. Then the cell is the same as before."
K=1.0; CW=140
SCRIBBLE='M 30,60 C 50,20 70,100 90,50 S 120,90 100,110 S 40,100 60,70'
Y=1115; AY=$((Y+CW/2))
cell 110 $Y "1 3" ""; ink 110 $Y "$SCRIBBLE"; arrow 265 335 $AY
filled 350 $Y "?" "" gray40; arrow 505 575 $AY
cell 590 $Y "1 3" ""
text 180 $((Y+CW+45)) 30 gray25 middle "Scribble"
text 420 $((Y+CW+45)) 30 gray25 middle "? for one second"
text 660 $((Y+CW+45)) 30 gray25 middle "No change"

K=1.25; CW=175
text 972 290 50 black start "One number per cell"
text 972 355 38 gray25 start "The app reads your writing when you stop"
text 972 405 38 gray25 start "for a short time. Thus you can write a"
text 972 455 38 gray25 start "number with two strokes, for example 4."
text 972 505 38 gray25 start "Writing on a number has no effect. Erase"
text 972 555 38 gray25 start "the number first."
SEVEN='M 40,35 L 102,32 L 62,112'; THREE='M 45,30 C 90,20 100,60 65,68 C 105,72 100,120 45,110'
for row in 0 1; do
  Y=$((600+row*330)); AY=$((Y+CW/2))
  if [ $row = 0 ]; then
    filled 1100 $Y 7 "$GIVEN_BG"; ink 1100 $Y "$THREE"; arrow 1290 1350 $AY; filled 1365 $Y 7 "$GIVEN_BG"
    text 1320 $((Y+CW+50)) 34 gray25 middle "Starting number: no change"
  else
    filled 1100 $Y 4 "" gray35; ink 1100 $Y "$SEVEN"; arrow 1290 1350 $AY; filled 1365 $Y 4 "" gray35
    text 1320 $((Y+CW+50)) 34 gray25 middle "Your number: no change"
  fi
done

pagenum 4 9
render "$1/manual-pen-rules-landscape.png"
