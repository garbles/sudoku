#!/bin/sh
# Makes manual-pen.png in $1: page 3 of the manual. It tells how to write and erase numbers with the pen.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
SIX='M 101,33 C 97,22 84,16 70,24 C 54,34 42,62 41,86 C 40,108 52,124 72,122 C 92,120 102,104 96,90 C 90,76 70,72 56,82 C 50,86 46,92 45,99'

text 72 170 84 black start "Write and erase"

text 72 300 50 black start "Write a number"
text 72 370 38 gray25 start "Use the pen to write a digit in an empty cell. The app reads"
text 72 420 38 gray25 start "your handwriting and shows the digit as a typed number."
text 72 470 38 gray25 start "Neat handwriting is not necessary. When a cell has a number,"
text 72 520 38 gray25 start "the app does not show its micro-cells and pencil marks."
Y=600; X1=254; X2=579; X3=904; AY=$((Y+CW/2))
cell $X1 $Y "1 3 5" ""
cell $X2 $Y "1 3 5" ""; ink $X2 $Y "$SIX"
filled $X3 $Y 6
arrow $((X1+CW+15)) $((X2-15)) $AY
arrow $((X2+CW+15)) $((X3-15)) $AY
CY=$((Y+CW+60))
text $((X1+CW/2)) $CY 34 gray25 middle "No number"
text $((X2+CW/2)) $CY 34 gray25 middle "Write a 6"
text $((X3+CW/2)) $CY 34 gray25 middle "The app shows 6"

text 72 1060 50 black start "Erase a number"
text 72 1130 38 gray25 start "Rub the cell with the eraser end of the pen. The app removes"
text 72 1180 38 gray25 start "the number. Then it shows the micro-cells and the pencil marks"
text 72 1230 38 gray25 start "of the cell again, if they are still possible."
Y=1310; AY=$((Y+CW/2))
filled $X1 $Y 6
filled $X2 $Y 6; rub $X2 $Y
cell $X3 $Y "1 3 5" ""
arrow $((X1+CW+15)) $((X2-15)) $AY
arrow $((X2+CW+15)) $((X3-15)) $AY
CY=$((Y+CW+60))
text $((X1+CW/2)) $CY 34 gray25 middle "Cell shows 6"
text $((X2+CW/2)) $CY 34 gray25 middle "Erase the cell"
text $((X3+CW/2)) $CY 34 gray25 middle "Pencil marks show"

pagenum 3 9
render "$1/manual-pen.png"
