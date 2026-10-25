#!/bin/sh
# Makes manual-helpers-landscape.png in $1: page 5 of the manual in landscape (the text of manual-helpers.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=1.25; CW=175
UI=$1

text 72 170 84 black start "Helpers"

text 72 290 50 black start "Automatic pencil mark removal"
text 72 355 38 gray25 start "When you write a number, the app removes"
text 72 405 38 gray25 start "that digit from the pencil marks in the"
text 72 455 38 gray25 start "same row, column and box."
text 72 505 38 gray25 start "Use undo to show the removed marks again."
FIVE='M 95,30 L 50,30 L 45,68 C 70,58 100,68 98,92 C 96,118 62,124 42,108'
Y=560
cell 150 $Y "3 5" ""; ink 150 $Y "$FIVE"; cell 325 $Y "1 5 7" ""; cell 500 $Y "5 9" ""
text 412 $((Y+CW+45)) 34 gray25 middle "Write a 5 in a row"
varrow 412 $((Y+CW+65)) $((Y+CW+145))
Y=$((Y+CW+165))
filled 150 $Y 5 "" gray35; cell 325 $Y "1 7" ""; cell 500 $Y "9" ""
text 412 $((Y+CW+45)) 34 gray25 middle "The app removes the other 5s"

text 972 290 50 black start "Highlight a number"
text 972 355 38 gray25 start "Tap a number with the pen. All cells with"
text 972 405 38 gray25 start "the same number become green. To remove"
text 972 455 38 gray25 start "the green, tap the number again or do a"
text 972 505 38 gray25 start "different action. Green is the default color."
picture_w 972 560 828 "$(shot "$UI/screen-highlight.png" 1336x894+142+412)"

pagenum 5 9
render "$1/manual-helpers-landscape.png"
