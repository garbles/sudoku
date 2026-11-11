#!/bin/sh
# Makes manual-new-game-landscape.png in $1: page 8 of the manual in landscape (the text of manual-new-game.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "New game"

step() { text 72 $1 50 black start "$2.  $3"; text 72 $(($1+60)) 38 gray25 start "$4"; }
step 290 1 "Open the menu" "Tap the menu button in the top right."
picture 1100 240 "$(shot "$UI/screen.png" $(slot_crop 0))"
step 430 2 "Tap +" "The + is at the bottom of the menu."
picture 1100 380 "$(shot "$UI/screen-toolbar.png" $(slot_crop new))"
step 570 3 "Select a difficulty" "Move the slider. Easy needs only simple logic."
text 72 680 38 gray25 start "Hard needs advanced logic. The slider starts at your last choice."
NGW=$((PHYS_W-NG_PL)); NGY=$((NG_TOP-4))   # the three-row panel (refer to manual-new-game.sh)
picture 160 715 "$(shot "$UI/new-game-menu.png" ${NGW}x$((PHYS_H-NGY))+$NG_PL+$NGY)"
text 72 1070 38 gray25 start "If Symmetrical is on, the starting numbers are the same after a"
text 72 1120 38 gray25 start "half turn of the board."
step 1210 4 "Tap Start new game" "The app shows a new puzzle at the difficulty that you selected."

text 1330 290 50 black start "Current difficulty"
text 1330 355 38 gray25 start "The meter below the board"
text 1330 405 38 gray25 start "shows the difficulty of the"
text 1330 455 38 gray25 start "current puzzle. You cannot"
text 1330 505 38 gray25 start "change it during a game."
DW=$((DIFF_W+80))
picture 1330 560 "$(shot "$UI/screen.png" ${DW}x70+$((X0+C*9/2-DW/2))+$((Y0+C*9+50-35)))"

pagenum 8 9
render "$1/manual-new-game-landscape.png"
