#!/bin/sh
# Makes manual-new-game.png in $1: page 8 of the manual. It tells how to start a new game, step by step.
set -e
. "$(dirname "$0")/manual-lib.sh"
. "$(dirname "$0")/ui-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "New game"

# step Y N HEADING DESCRIPTION
step() { text 72 $1 50 black start "$2.  $3"; text 72 $(($1+60)) 38 gray25 start "$4"; }
step 300 1 "Open the menu" "Tap the menu button in the top right."
picture 1235 250 "$(shot "$UI/screen.png" 112x112+1508+0)"
step 460 2 "Tap +" "The + is at the bottom of the menu."
picture 1235 410 "$(shot "$UI/screen-toolbar.png" 112x112+1508+2048)"
step 620 3 "Select a difficulty" "Move the slider. Easy needs only simple logic."
text 72 730 38 gray25 start "Hard needs advanced logic. The slider starts at your last choice."
# The New game panel has three rows (slider, Symmetrical, Start new game): its top is 226 px above the bottom
# toolbar slot (NG_TOP of ui-lib.sh). The crop starts 4 px above the panel top.
NGW=$((1620-NG_PL)); NGY=$((NG_TOP-4))
picture $(awk -v w=$NGW -v s=$SCALE 'BEGIN{printf "%d", 702-w/s/2}') 790 "$(shot "$UI/new-game-menu.png" ${NGW}x$((2160-NGY))+$NG_PL+$NGY)"
text 72 1150 38 gray25 start "If Symmetrical is on, the starting numbers are the same after a"
text 72 1200 38 gray25 start "half turn of the board."
step 1300 4 "Tap Start new game" "The app shows a new puzzle at the difficulty that you selected."

text 72 1470 50 black start "Current difficulty"
text 72 1540 38 gray25 start "The meter below the board shows the difficulty of the"
text 72 1590 38 gray25 start "current puzzle. You cannot change it during a game."
DW=$((DIFF_W+80))
picture $(awk -v w=$DW -v s=$SCALE 'BEGIN{printf "%d", 702-w/s/2}') 1640 "$(shot "$UI/screen.png" ${DW}x70+$((X0+C*9/2-DW/2))+$((Y0+C*9+50-35)))"

pagenum 8 9
render "$1/manual-new-game.png"
