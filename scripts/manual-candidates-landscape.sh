#!/bin/sh
# Makes manual-candidates-landscape.png in $1: page 2 of the manual in landscape (the text of manual-candidates.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "Pencil marks"

text 72 290 50 black start "Micro-cells"
text 72 355 38 gray25 start "An empty cell has nine faint"
text 72 405 38 gray25 start "micro-cells. Each micro-cell is"
text 72 455 38 gray25 start "for one digit. The digits have"
text 72 505 38 gray25 start "the same positions as the keys"
text 72 555 38 gray25 start "on a phone keypad."
cell 640 270 "1 2 3 4 5 6 7 8 9" "" gray55

text 72 700 50 black start "Remove a pencil mark"
text 72 765 38 gray25 start "Tap a micro-cell that shows a digit."
text 72 815 38 gray25 start "The app removes the digit."
K=1.25; CW=175
Y=860; X1=150; X2=500; AY=$((Y+CW/2))
cell $X1 $Y "1 3 5" 5
cell $X2 $Y "1 3" ""
arrow $((X1+CW+15)) $((X2-15)) $AY
CY=$((Y+CW+50))
text $((X1+CW/2)) $CY 34 gray25 middle "Tap the 5"
text $((X2+CW/2)) $CY 34 gray25 middle "No 5"

text 972 290 50 black start "Add a pencil mark"
text 972 355 38 gray25 start "Tap a micro-cell. The app adds its digit as a"
text 972 405 38 gray25 start "pencil mark."
X1=1050; X2=1450
for row in 0 1; do
  Y=$((450+row*330)); AY=$((Y+CW/2)); CY=$((Y+CW+50))
  if [ $row = 0 ]; then
    cell $X1 $Y "" 1; cell $X2 $Y "1" 3
    text $((X1+CW/2)) $CY 34 gray25 middle "Tap top left"; text $((X2+CW/2)) $CY 34 gray25 middle "Tap top right"
  else
    cell $X1 $Y "1 3" 5; cell $X2 $Y "1 3 5" ""
    text $((X1+CW/2)) $CY 34 gray25 middle "Tap middle"; text $((X2+CW/2)) $CY 34 gray25 middle "Result: 1, 3, 5"
  fi
  arrow $((X1+CW+15)) $((X2-15)) $AY
done

text 936 1250 32 gray40 middle "You can add pencil marks only to cells that do not have a number."

pagenum 2 9
render "$1/manual-candidates-landscape.png"
