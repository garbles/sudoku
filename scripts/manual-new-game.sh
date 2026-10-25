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
step 620 3 "Select a difficulty" "Move the slider. Easy has the most starting numbers."
text 72 730 38 gray25 start "Insane has the fewest. The slider starts at your last choice."
NGW=$((1620-NG_PL))
picture $(awk -v w=$NGW -v s=$SCALE 'BEGIN{printf "%d", 702-w/s/2}') 790 "$(shot "$UI/new-game-menu.png" ${NGW}x228+$NG_PL+1932)"
step 1150 4 "Tap Start new game" "The app shows a new puzzle at the difficulty that you selected."

text 72 1400 50 black start "Current difficulty"
text 72 1470 38 gray25 start "The meter below the board shows the difficulty of the"
text 72 1520 38 gray25 start "current puzzle. You cannot change it during a game."
DW=$((DIFF_W+80))
picture $(awk -v w=$DW -v s=$SCALE 'BEGIN{printf "%d", 702-w/s/2}') 1580 "$(shot "$UI/screen.png" ${DW}x70+$((X0+C*9/2-DW/2))+$((Y0+C*9+50-35)))"

pagenum 8 9
render "$1/manual-new-game.png"
