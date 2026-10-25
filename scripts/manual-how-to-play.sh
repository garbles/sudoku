#!/bin/sh
# Makes manual-how-to-play.png in $1: page 1 of the manual. It tells about the rules of sudoku and the buttons.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "How to play"

text 72 300 50 black start "The goal"
text 72 370 38 gray25 start "Write a digit in each empty cell. Each row, each column and"
text 72 420 38 gray25 start "each 3x3 box must have the digits 1 to 9 one time."
ROW="3 7 1 9 5 2 8 6 4"
hl=""; dg=""; i=0; for d in $ROW; do hl="$hl 4,$i,$UNIT_BG"; dg="$dg 4,$i,$d,black"; i=$((i+1)); done
minib 153 480 "$hl" "$dg"
hl=""; dg=""; i=0; for d in $ROW; do hl="$hl $i,4,$UNIT_BG"; dg="$dg $i,4,$d,black"; i=$((i+1)); done
minib 549 480 "$hl" "$dg"
hl=""; dg=""; i=0; for d in $ROW; do r=$((3+i/3)); c=$((6+i%3)); hl="$hl $r,$c,$UNIT_BG"; dg="$dg $r,$c,$d,black"; i=$((i+1)); done
minib 945 480 "$hl" "$dg"
text 306 840 34 gray25 middle "Each row"
text 702 840 34 gray25 middle "Each column"
text 1098 840 34 gray25 middle "Each box"

text 72 960 50 black start "Starting numbers"
text 72 1030 38 gray25 start "The game starts with some numbers on the board. These"
text 72 1080 38 gray25 start "numbers have a color background. Gray is the default."
filled 417 1120 7 $GIVEN_BG
filled 742 1120 4 "" $ENTRY_FG
text 539 1425 34 gray25 middle "Starting number"
text 864 1425 34 gray25 middle "Your number"

text 72 1540 50 black start "Buttons"
echo "fill none stroke black stroke-width 5 stroke-linecap round line 103,1599 121,1617 line 121,1599 103,1617" >> $M
echo "fill none stroke black stroke-width 5 circle 112,1608 140,1608" >> $M
text 180 1622 38 gray25 start "Close the game (top left, only when the menu is open)"
echo "fill none stroke black stroke-width 5 circle 112,1698 140,1698 fill black stroke none circle 103,1707 109,1707" >> $M
text 180 1712 38 gray25 start "Open the menu (top right). Page 7 tells about the menu."

text 702 1785 30 gray40 middle "To turn the page, swipe left or right with your finger, or tap an arrow."
pagenum 1 9
render "$1/manual-how-to-play.png"
