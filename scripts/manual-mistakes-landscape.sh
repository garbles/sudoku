#!/bin/sh
# Makes manual-mistakes-landscape.png in $1: page 6 of the manual in landscape (the text of manual-mistakes.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "Mistakes"

text 72 290 50 black start "Clashing numbers"
text 72 355 38 gray25 start "If you write a number that is already in the same row, column or box, the two numbers"
text 72 405 38 gray25 start "become red (the default color). This is a clash."
minib 387 450 "4,1,$CLASH_BG 4,6,$CLASH_BG 0,0,$GIVEN_BG 2,4,$GIVEN_BG 7,7,$GIVEN_BG" \
  "4,1,5,black 4,6,5,$ENTRY_FG 0,0,8,black 2,4,3,black 7,7,1,black"
minib 783 450 "1,4,$CLASH_BG 7,4,$CLASH_BG 0,8,$GIVEN_BG 4,1,$GIVEN_BG 6,6,$GIVEN_BG" \
  "1,4,2,black 7,4,2,$ENTRY_FG 0,8,6,black 4,1,9,black 6,6,4,black"
minib 1179 450 "3,6,$CLASH_BG 5,8,$CLASH_BG 1,1,$GIVEN_BG 4,3,$GIVEN_BG 8,0,$GIVEN_BG" \
  "3,6,8,black 5,8,8,$ENTRY_FG 1,1,7,black 4,3,1,black 8,0,3,black"
text 540 805 34 gray25 middle "Same row"
text 936 805 34 gray25 middle "Same column"
text 1332 805 34 gray25 middle "Same box"

text 72 910 50 black start "Correct a mistake"
text 72 975 38 gray25 start "Starting numbers are always correct. Thus you"
text 72 1025 38 gray25 start "must change your number. Erase it with the pen,"
text 72 1075 38 gray25 start "or use undo. When there is no clash, the app"
text 72 1125 38 gray25 start "removes the red color."

text 972 910 50 black start "The app shows only clashes"
text 972 975 38 gray25 start "The app does not compare your numbers with"
text 972 1025 38 gray25 start "the solution. Thus a number can be incorrect"
text 972 1075 38 gray25 start "and not red."

# The correction (manual-mistakes.sh): your 9 clashes with a starting 9; erase it, then there is no clash.
K=1.0; CW=140
Y=1150; AY=$((Y+CW/2))
filled 100 $Y 9 "$CLASH_BG"
filled 260 $Y 9 "$CLASH_BG" $ENTRY_FG; rub 260 $Y
arrow 420 480 $AY
filled 500 $Y 9 $GIVEN_BG
cell 660 $Y "" ""
text 250 $((Y+CW+40)) 30 gray25 middle "Erase your 9"
text 650 $((Y+CW+40)) 30 gray25 middle "No clash"

pagenum 6 9
render "$1/manual-mistakes-landscape.png"
