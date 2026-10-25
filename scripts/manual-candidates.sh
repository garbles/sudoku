#!/bin/sh
# Makes manual-candidates.png in $1: page 2 of the manual. It tells how to tap micro-cells (pencil marks).
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "Pencil marks"

text 72 300 50 black start "Micro-cells"
text 72 370 38 gray25 start "An empty cell has nine faint micro-cells."
text 72 420 38 gray25 start "Each micro-cell is for one digit. The digits"
text 72 470 38 gray25 start "have the same positions as the keys on a"
text 72 520 38 gray25 start "phone keypad."
cell 1087 280 "1 2 3 4 5 6 7 8 9" "" gray55

text 72 680 50 black start "Add a pencil mark"
text 72 750 38 gray25 start "Tap a micro-cell. The app adds its digit as a pencil mark."
Y=810; X1=92; X2=417; X3=742; X4=1067; AY=$((Y+CW/2))
cell $X1 $Y "" 1
cell $X2 $Y "1" 3
cell $X3 $Y "1 3" 5
cell $X4 $Y "1 3 5" ""
arrow $((X1+CW+15)) $((X2-15)) $AY
arrow $((X2+CW+15)) $((X3-15)) $AY
arrow $((X3+CW+15)) $((X4-15)) $AY
CY=$((Y+CW+60))
text $((X1+CW/2)) $CY 34 gray25 middle "Tap top left"
text $((X2+CW/2)) $CY 34 gray25 middle "Tap top right"
text $((X3+CW/2)) $CY 34 gray25 middle "Tap middle"
text $((X4+CW/2)) $CY 34 gray25 middle "Result: 1, 3, 5"

text 72 1250 50 black start "Remove a pencil mark"
text 72 1320 38 gray25 start "Tap a micro-cell that shows a digit. The app removes the digit."
Y=1380; X1=417; X2=742; AY=$((Y+CW/2))
cell $X1 $Y "1 3 5" 5
cell $X2 $Y "1 3" ""
arrow $((X1+CW+15)) $((X2-15)) $AY
CY=$((Y+CW+60))
text $((X1+CW/2)) $CY 34 gray25 middle "Tap the 5"
text $((X2+CW/2)) $CY 34 gray25 middle "No 5"

text 702 1790 32 gray40 middle "You can add pencil marks only to cells that do not have a number."

pagenum 2 9
render "$1/manual-candidates.png"
