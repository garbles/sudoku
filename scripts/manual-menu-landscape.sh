#!/bin/sh
# Makes manual-menu-landscape.png in $1: page 7 of the manual in landscape (the text of manual-menu.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "The menu"
text 72 255 38 gray25 start "Tap the menu button in the top right to open the menu."

# row Y ICON HEADING DESCRIPTION : a toolbar icon at its real size, with a heading and one line of description
row() {
  picture 72 $1 "$2"
  text 200 $(($1+42)) 40 black start "$3"
  text 200 $(($1+88)) 34 gray25 start "$4"
}
slot() { shot "$UI/screen-toolbar.png" $(slot_crop $1); }
slot_on() { shot "$UI/color-menu.png" $(slot_crop $1); }
row 300 "$(slot 0)" "Menu" "Opens and closes the menu."
row 410 "$(slot 1)" "Cell colors" "Sets the colors of numbers and completed groups."
row 520 "$(slot 2)" "Settings" "Turns the helpers on or off. Shows the"
row 664 "$(slot_on 3)" "Undo" "Cancels your last change to the board."
row 774 "$(slot_on 4)" "Redo" "Makes the last canceled change again."
row 884 "$(slot 5)" "Checkpoints" "Saves the board. Restore shows it again."
row 994 "$(slot help)" "Help" "Opens this manual."
row 1104 "$(slot statistics)" "Statistics" "Shows your completed puzzles and their times."
row 1214 "$(slot new)" "New game" "Starts a new puzzle. Refer to page 8."
text 200 $((520+128)) 34 gray25 start "version and the licenses."

text 1010 330 50 black start "Select colors"
text 1010 395 38 gray25 start "Tap Cell colors to open the panel. It has"
text 1010 445 38 gray25 start "five sections: starting, clashing and"
text 1010 495 38 gray25 start "highlighted numbers, completed groups and"
text 1010 545 38 gray25 start "micro-cell lines. In each section, tap a"
text 1010 595 38 gray25 start "color. A black box shows the selected"
text 1010 645 38 gray25 start "color. A line through a color shows that"
text 1010 695 38 gray25 start "it is in use. White in Completed groups"
text 1010 745 38 gray25 start "gives no completed color."
picture 1010 790 "$(shot "$UI/color-menu.png" $COLOR_PANEL_CROP)"

pagenum 7 9
render "$1/manual-menu-landscape.png"
