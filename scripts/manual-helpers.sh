#!/bin/sh
# Makes manual-helpers.png in $1: page 5 of the manual. It tells about automatic pencil mark hiding, highlights and completed groups.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=1.0; CW=140
UI=$1

text 72 170 84 black start "Helpers"

text 72 270 50 black start "Automatic pencil mark hiding"
text 72 330 38 gray25 start "When you write a number, the app hides that digit in the pencil marks"
text 72 380 38 gray25 start "in the same row, column and box. Erase the number to show them again."
text 72 430 38 gray25 start "If you tap the micro-cell of a hidden digit, the tap has no effect and"
text 72 480 38 gray25 start "a small x shows there for a moment. Settings can turn it off."
FIVE='M 95,30 L 50,30 L 45,68 C 70,58 100,68 98,92 C 96,118 62,124 42,108'
Y=510; AY=$((Y+CW/2))
cell 220 $Y "3 5" ""; ink 220 $Y "$FIVE"; cell 360 $Y "1 5 7" ""; cell 500 $Y "5 9" ""
arrow 665 745 $AY
filled 765 $Y 5 "" $ENTRY_FG; cell 905 $Y "1 7" ""; cell 1045 $Y "9" ""
text 430 $((Y+CW+40)) 34 gray25 middle "Write a 5 in a row"
text 975 $((Y+CW+40)) 34 gray25 middle "The app hides the other 5s"

text 72 780 50 black start "Highlight a number"
text 72 840 38 gray25 start "Tap a number with the pen. All"
text 72 890 38 gray25 start "cells with the same number get"
text 72 940 38 gray25 start "the highlight color (cyan is the"
text 72 990 38 gray25 start "default). To remove it, tap or"
text 72 1040 38 gray25 start "write on the board with the pen."
text 72 1090 38 gray25 start "Settings can turn it off."
picture_w 724 750 608 "$(shot "$UI/screen-highlight.png" 1336x743+142+412)"

text 72 1165 50 black start "Completed groups"
text 72 1225 38 gray25 start "A group is a row, a column or a 3x3 box. When a group is full and has no"
text 72 1275 38 gray25 start "clash, all its cells get the completed color (green is the default). A clash"
text 72 1325 38 gray25 start "with a cell outside the group also stops the color. The color is not a"
text 72 1375 38 gray25 start "check against the solution: a group with the color can have an incorrect"
text 72 1425 38 gray25 start "number. Settings can turn it off."
completed_examples 153 549 945 1450

pagenum 5 9
render "$1/manual-helpers.png"
