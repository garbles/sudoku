#!/bin/sh
# Makes manual-new-game-landscape.png in $1: page 8 of the manual in landscape (the text of manual-new-game.sh).
set -e
. "$(dirname "$0")/ui-lib.sh"
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "New game"

step() { text 72 $1 50 black start "$2.  $3"; text 72 $(($1+60)) 38 gray25 start "$4"; }
step 290 1 "Open the menu" "Tap the menu button in the top right."
picture 1100 240 "$(shot "$UI/screen.png" 112x112+1508+0)"
step 430 2 "Tap +" "The + is at the bottom of the menu."
picture 1100 380 "$(shot "$UI/screen-toolbar.png" 112x112+1508+2048)"
step 570 3 "Select a difficulty" "Move the slider. Easy has the most starting numbers."
text 72 680 38 gray25 start "Hard has the fewest. The slider starts at your last choice."
NGW=$((1620-NG_PL))
picture 160 720 "$(shot "$UI/new-game-menu.png" ${NGW}x228+$NG_PL+1932)"
step 1000 4 "Tap Start new game" "The app shows a new puzzle at the difficulty that you selected."

text 1330 290 50 black start "Current difficulty"
text 1330 355 38 gray25 start "The meter below the board"
text 1330 405 38 gray25 start "shows the difficulty of the"
text 1330 455 38 gray25 start "current puzzle. You cannot"
text 1330 505 38 gray25 start "change it during a game."
DW=$((DIFF_W+80))
picture 1330 560 "$(shot "$UI/screen.png" ${DW}x70+$((X0+C*9/2-DW/2))+$((Y0+C*9+50-35)))"

pagenum 8 9
render "$1/manual-new-game-landscape.png"
