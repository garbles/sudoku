#!/bin/sh
# Makes manual-how-to-play-landscape.png in $1: page 1 of the manual in landscape (the text of manual-how-to-play.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)

text 72 170 84 black start "How to play"

text 72 290 50 black start "The goal"
text 72 355 38 gray25 start "Write a digit in each empty cell. Each row, each column and each 3x3 box must have the"
text 72 405 38 gray25 start "digits 1 to 9 one time."
ROW="3 7 1 9 5 2 8 6 4"
hl=""; dg=""; i=0; for d in $ROW; do hl="$hl 4,$i,$UNIT_BG"; dg="$dg 4,$i,$d,black"; i=$((i+1)); done
minib 387 450 "$hl" "$dg"
hl=""; dg=""; i=0; for d in $ROW; do hl="$hl $i,4,$UNIT_BG"; dg="$dg $i,4,$d,black"; i=$((i+1)); done
minib 783 450 "$hl" "$dg"
hl=""; dg=""; i=0; for d in $ROW; do r=$((3+i/3)); c=$((6+i%3)); hl="$hl $r,$c,$UNIT_BG"; dg="$dg $r,$c,$d,black"; i=$((i+1)); done
minib 1179 450 "$hl" "$dg"
text 540 805 34 gray25 middle "Each row"
text 936 805 34 gray25 middle "Each column"
text 1332 805 34 gray25 middle "Each box"

text 72 910 50 black start "Starting numbers"
text 72 975 38 gray25 start "The game starts with some numbers on the"
text 72 1025 38 gray25 start "board. These numbers have a color"
text 72 1075 38 gray25 start "background. Gray is the default."
K=1.0; CW=140
filled 160 1110 7 $GIVEN_BG
filled 400 1110 4 "" $ENTRY_FG
text 230 1290 30 gray25 middle "Starting number"
text 470 1290 30 gray25 middle "Your number"

text 972 910 50 black start "Buttons"
echo "fill none stroke black stroke-width 5 stroke-linecap round line 1003,969 1021,987 line 1021,969 1003,987" >> $M
echo "fill none stroke black stroke-width 5 circle 1012,978 1040,978" >> $M
text 1080 992 38 gray25 start "Close the game (top left, only when"
text 1080 1042 38 gray25 start "the menu is open)"
echo "fill none stroke black stroke-width 5 circle 1012,1118 1040,1118 fill black stroke none circle 1003,1127 1009,1127" >> $M
text 1080 1132 38 gray25 start "Open the menu (top right). Page 7"
text 1080 1182 38 gray25 start "tells about the menu."

text 1386 1290 30 gray40 middle "To turn the page, swipe left or right, or tap an arrow."
pagenum 1 9
render "$1/manual-how-to-play-landscape.png"
