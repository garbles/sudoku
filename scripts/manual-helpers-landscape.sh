#!/bin/sh
# Makes manual-helpers-landscape.png in $1: page 5 of the manual in landscape (the text of manual-helpers.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=0.75; CW=105
UI=$1

text 72 170 84 black start "Helpers"

text 72 270 50 black start "Automatic pencil mark hiding"
text 72 330 38 gray25 start "When you write a number, the app hides that"
text 72 380 38 gray25 start "digit in the pencil marks in the same row,"
text 72 430 38 gray25 start "column and box. Erase the number to show"
text 72 480 38 gray25 start "them again. If you tap the micro-cell of a"
text 72 530 38 gray25 start "hidden digit, the tap has no effect and a"
text 72 580 38 gray25 start "small x shows there for a moment. Settings"
text 72 630 38 gray25 start "can turn it off."
FIVE='M 95,30 L 50,30 L 45,68 C 70,58 100,68 98,92 C 96,118 62,124 42,108'
Y=660; AY=$((Y+CW/2))
cell 90 $Y "3 5" ""; ink 90 $Y "$FIVE"; cell 195 $Y "1 5 7" ""; cell 300 $Y "5 9" ""
arrow 420 490 $AY
filled 505 $Y 5 "" $ENTRY_FG; cell 610 $Y "1 7" ""; cell 715 $Y "9" ""
text 247 $((Y+CW+40)) 34 gray25 middle "Write a 5 in a row"
text 662 $((Y+CW+40)) 34 gray25 middle "The app hides the other 5s"

text 972 270 50 black start "Highlight a number"
text 972 330 38 gray25 start "Tap a number with the pen. All cells with"
text 972 380 38 gray25 start "the same number get the highlight color"
text 972 430 38 gray25 start "(cyan is the default). To remove it, tap or"
text 972 480 38 gray25 start "write on the board with the pen."
text 972 530 38 gray25 start "Settings can turn it off."
picture_w 1076 555 620 "$(shot "$UI/screen-highlight.png" $HELPERS_CROP_LANDSCAPE)"

text 72 880 50 black start "Completed groups"
text 72 940 38 gray25 start "A group is a row, a column or a 3x3"
text 72 990 38 gray25 start "box. When a group is full and has no"
text 72 1040 38 gray25 start "clash, all its cells get the completed"
text 72 1090 38 gray25 start "color (green is the default). A clash"
text 72 1140 38 gray25 start "with a cell outside the group also"
text 72 1190 38 gray25 start "stops the color. The color is not a"
text 72 1240 38 gray25 start "check against the solution: a group"
text 72 1290 38 gray25 start "with the color can have an incorrect"
text 72 1340 38 gray25 start "number. Settings can turn it off."
completed_examples 820 1155 1490 900

pagenum 5 9
render "$1/manual-helpers-landscape.png"
