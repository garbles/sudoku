#!/bin/sh
# Makes manual-helpers.png in $1: page 5 of the manual. It tells about automatic pencil mark removal and highlights.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
K=1.25; CW=175
UI=$1

text 72 170 84 black start "Helpers"

text 72 290 50 black start "Automatic pencil mark removal"
text 72 355 38 gray25 start "When you write a number, the app removes that digit from the"
text 72 405 38 gray25 start "pencil marks in the same row, column and box."
text 72 455 38 gray25 start "Use undo to show them again. Settings can turn it off."
FIVE='M 95,30 L 50,30 L 45,68 C 70,58 100,68 98,92 C 96,118 62,124 42,108'
Y=510; AY=$((Y+CW/2))
cell 117 $Y "3 5" ""; ink 117 $Y "$FIVE"; cell 292 $Y "1 5 7" ""; cell 467 $Y "5 9" ""
arrow 662 742 $AY
filled 762 $Y 5 "" gray35; cell 937 $Y "1 7" ""; cell 1112 $Y "9" ""
text 379 $((Y+CW+50)) 34 gray25 middle "Write a 5 in a row"
text 1024 $((Y+CW+50)) 34 gray25 middle "The app removes the other 5s"

text 72 830 50 black start "Highlight a number"
text 72 890 38 gray25 start "Tap a number with the pen. All cells with the same number"
text 72 940 38 gray25 start "become green. To remove the green, tap the number again or"
text 72 990 38 gray25 start "do a different action. Settings can turn it off."
picture 123 1022 "$(shot "$UI/screen-highlight.png" 1336x894+142+412)"

pagenum 5 9
render "$1/manual-helpers.png"
