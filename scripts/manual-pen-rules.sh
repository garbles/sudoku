#!/bin/sh
# Makes manual-pen-rules.png in $1: page 4 of the manual. It tells what a tap is, which writing has no effect,
# and what occurs when the app cannot read the writing.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=1.25; CW=175   # smaller example cells on this page

text 72 170 84 black start "Pen and touch"

text 72 290 50 black start "Taps"
text 72 355 38 gray25 start "A tap is a pen touch that stays in one micro-cell. If the pen"
text 72 405 38 gray25 start "moves out of the micro-cell, the touch is writing, not a tap."
text 72 455 38 gray25 start "Finger touches on the board have no effect. Use the pen."
cell 400 500 "" 1
cell 830 500 "" ""
open_cell 830 500
echo "fill none stroke black stroke-width 3 circle 23,23 31,23 stroke-linecap round path 'M 23,23 L 72,74 M 60,72 L 72,74 L 70,62'" >> $M
echo "pop graphic-context" >> $M
text 487 725 34 gray25 middle "Tap"
text 917 725 34 gray25 middle "Not a tap"

text 72 830 50 black start "One number per cell"
text 72 895 38 gray25 start "The app reads your writing when you stop for a short time."
text 72 945 38 gray25 start "Thus you can write a number with two strokes, for example 4."
text 72 995 38 gray25 start "Writing on a number has no effect. Erase the number first."
SEVEN='M 40,35 L 102,32 L 62,112'; THREE='M 45,30 C 90,20 100,60 65,68 C 105,72 100,120 45,110'
Y=1040; AY=$((Y+CW/2))
filled 120 $Y 7 gray88; ink 120 $Y "$THREE"; arrow 305 365 $AY; filled 380 $Y 7 gray88
filled 850 $Y 4 "" gray35; ink 850 $Y "$SEVEN"; arrow 1035 1095 $AY; filled 1110 $Y 4 "" gray35
text 337 $((Y+CW+50)) 34 gray25 middle "Starting number: no change"
text 1067 $((Y+CW+50)) 34 gray25 middle "Your number: no change"

text 72 1370 50 black start "Not a digit"
text 72 1435 38 gray25 start "If you write a mark that is not a number from 1 to 9, the app"
text 72 1485 38 gray25 start "shows ? for one second. Then the cell is the same as before."
SCRIBBLE='M 30,60 C 50,20 70,100 90,50 S 120,90 100,110 S 40,100 60,70'
Y=1530; AY=$((Y+CW/2))
cell 300 $Y "1 3" ""; ink 300 $Y "$SCRIBBLE"; arrow 490 580 $AY
filled 595 $Y "?" "" gray40; arrow 785 875 $AY
cell 890 $Y "1 3" ""
text 387 $((Y+CW+50)) 34 gray25 middle "Scribble"
text 682 $((Y+CW+50)) 34 gray25 middle "? for one second"
text 977 $((Y+CW+50)) 34 gray25 middle "No change"

pagenum 4 9
render "$1/manual-pen-rules.png"
