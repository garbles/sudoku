#!/bin/sh
# Makes manual-pen-landscape.png in $1: page 3 of the manual in landscape (the text of manual-pen.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
SIX='M 101,33 C 97,22 84,16 70,24 C 54,34 42,62 41,86 C 40,108 52,124 72,122 C 92,120 102,104 96,90 C 90,76 70,72 56,82 C 50,86 46,92 45,99'

text 72 170 84 black start "Write and erase"
K=1.25; CW=175

text 72 290 50 black start "Write a number"
text 72 355 38 gray25 start "Use the pen to write a digit in an empty"
text 72 405 38 gray25 start "cell. The app reads your handwriting and"
text 72 455 38 gray25 start "shows the digit as a typed number."
text 72 505 38 gray25 start "Neat handwriting is not necessary. When"
text 72 555 38 gray25 start "a cell has a number, the app does not"
text 72 605 38 gray25 start "show its micro-cells and pencil marks."
Y=700; X1=90; X2=380; X3=670; AY=$((Y+CW/2))
cell $X1 $Y "1 3 5" ""
cell $X2 $Y "1 3 5" ""; ink $X2 $Y "$SIX"
filled $X3 $Y 6
arrow $((X1+CW+15)) $((X2-15)) $AY
arrow $((X2+CW+15)) $((X3-15)) $AY
CY=$((Y+CW+50))
text $((X1+CW/2)) $CY 34 gray25 middle "No number"
text $((X2+CW/2)) $CY 34 gray25 middle "Write a 6"
text $((X3+CW/2)) $CY 34 gray25 middle "The app shows 6"

text 972 290 50 black start "Erase a number"
text 972 355 38 gray25 start "Rub the cell with the eraser end of the"
text 972 405 38 gray25 start "pen. The app removes the number. Then"
text 972 455 38 gray25 start "it shows the micro-cells and the pencil"
text 972 505 38 gray25 start "marks of the cell again, if they are still"
text 972 555 38 gray25 start "possible."
X1=990; X2=1280; X3=1570
filled $X1 $Y 6
filled $X2 $Y 6; rub $X2 $Y
cell $X3 $Y "1 3 5" ""
arrow $((X1+CW+15)) $((X2-15)) $AY
arrow $((X2+CW+15)) $((X3-15)) $AY
text $((X1+CW/2)) $CY 34 gray25 middle "Cell shows 6"
text $((X2+CW/2)) $CY 34 gray25 middle "Erase the cell"
text $((X3+CW/2)) $CY 34 gray25 middle "Pencil marks show"

pagenum 3 9
render "$1/manual-pen-landscape.png"
