#!/bin/sh
# Makes manual-menu-landscape.png in $1: page 7 of the manual in landscape (the text of manual-menu.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "The menu"
text 72 245 38 gray25 start "Tap the menu button in the top right to open the menu."

# row Y ICON HEADING LINE... : a toolbar icon at its real size, with a heading and one or more lines of description.
# The rows have a step of 89 units, and each more line adds 40 units. The icon picture is 97 units high: the white
# edge of the next picture goes on the white edge of this one.
row() {
  picture 72 $1 "$2"
  text 200 $(($1+37)) 40 black start "$3"
  ry=$(($1+77)); shift 3
  for line in "$@"; do text 200 $ry 34 gray25 start "$line"; ry=$((ry+40)); done
}
slot() { shot "$UI/screen-toolbar.png" $(slot_crop $1); }
# The top slots: 0 menu, 1 hint, 2 cell colors, 3 settings, 4 undo, 5 redo, 6 checkpoints.
slot_on() { shot "$UI/color-menu.png" $(slot_crop $1); }
row 260 "$(slot 0)" "Menu" "Opens and closes the menu."
row 349 "$(slot 1)" "Hint" "Reveals one number. The number has the highlight" \
  "color until your next action on the board. The" \
  "button shows the quantity of hints that you used." \
  "No hint is available while numbers clash."
row 558 "$(slot 2)" "Cell colors" "Sets the colors of numbers and completed groups."
row 647 "$(slot 3)" "Settings" "Turns the helpers on or off. Shows the" "version and the licenses."
row 776 "$(slot_on 4)" "Undo" "Cancels your last change to the board."
row 865 "$(slot_on 5)" "Redo" "Makes the last canceled change again."
row 954 "$(slot 6)" "Checkpoints" "Saves the board. Restore shows it again."
row 1043 "$(slot help)" "Help" "Opens this manual."
row 1132 "$(slot statistics)" "Statistics" "Shows your completed puzzles and their times."
row 1221 "$(slot new)" "New game" "Starts a new puzzle. Refer to page 8."

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
