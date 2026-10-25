#!/bin/sh
# Makes manual-mistakes.png in $1: page 6 of the manual. It tells how the app shows clashes and how to correct them.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "Mistakes"

text 72 300 50 black start "Clashing numbers"
text 72 370 38 gray25 start "If you write a number that is already in the same row,"
text 72 420 38 gray25 start "column or box, the two numbers become red (the default"
text 72 470 38 gray25 start "color). This is a clash."
# row: starting 5 at (4,1), player 5 at (4,6)
minib 153 530 "4,1,$CLASH_BG 4,6,$CLASH_BG 0,0,$GIVEN_BG 2,4,$GIVEN_BG 7,7,$GIVEN_BG" \
  "4,1,5,black 4,6,5,$ENTRY_FG 0,0,8,black 2,4,3,black 7,7,1,black"
# column: starting 2 at (1,4), player 2 at (7,4)
minib 549 530 "1,4,$CLASH_BG 7,4,$CLASH_BG 0,8,$GIVEN_BG 4,1,$GIVEN_BG 6,6,$GIVEN_BG" \
  "1,4,2,black 7,4,2,$ENTRY_FG 0,8,6,black 4,1,9,black 6,6,4,black"
# box: starting 8 at (3,6), player 8 at (5,8)
minib 945 530 "3,6,$CLASH_BG 5,8,$CLASH_BG 1,1,$GIVEN_BG 4,3,$GIVEN_BG 8,0,$GIVEN_BG" \
  "3,6,8,black 5,8,8,$ENTRY_FG 1,1,7,black 4,3,1,black 8,0,3,black"
text 306 890 34 gray25 middle "Same row"
text 702 890 34 gray25 middle "Same column"
text 1098 890 34 gray25 middle "Same box"

text 72 1010 50 black start "Correct a mistake"
text 72 1080 38 gray25 start "Starting numbers are always correct. Thus you must change"
text 72 1130 38 gray25 start "your number. Erase it with the pen, or use undo. When there"
text 72 1180 38 gray25 start "is no clash, the app removes the red color."
Y=1240; AY=$((Y+CW/2))
filled 132 $Y 9 "$CLASH_BG"
filled 397 $Y 9 "$CLASH_BG" $ENTRY_FG; rub 397 $Y
arrow 662 742 $AY
filled 762 $Y 9 $GIVEN_BG
cell 1027 $Y "" ""
text 387 $((Y+CW+60)) 34 gray25 middle "Erase your 9"
text 1017 $((Y+CW+60)) 34 gray25 middle "No clash"

text 72 1640 50 black start "The app shows only clashes"
text 72 1710 38 gray25 start "The app does not compare your numbers with the solution."
text 72 1760 38 gray25 start "Thus a number can be incorrect and not red."

pagenum 6 9
render "$1/manual-mistakes.png"
