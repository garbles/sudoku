#!/bin/sh
# Makes manual-menu.png in $1: page 7 of the manual. It tells about each menu button and how to select a color.
# The icons and the color panel are parts of the UI images. They show at their real size.
set -e
. "$(dirname "$0")/manual-lib.sh"
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
slot_on() { shot "$UI/color-menu.png" $(slot_crop $1); }   # undo and redo are available, as in a game in progress
row 260 "$(slot 0)" "Menu" "Opens and closes the menu."
row 349 "$(slot 1)" "Hint" "Reveals one number. The number has the highlight color until your next" \
  "action on the board. The button shows the quantity of hints that you" \
  "used. No hint is available while numbers clash."
row 518 "$(slot 2)" "Cell colors" "Sets the colors of numbers and completed groups."
row 607 "$(slot 3)" "Settings" "Turns the helpers on or off. Shows the version and the licenses."
row 696 "$(slot_on 4)" "Undo" "Cancels your last change to the board."
row 785 "$(slot_on 5)" "Redo" "Makes the last canceled change again."
row 874 "$(slot 6)" "Checkpoints" "Saves the board. Restore shows it again."
row 963 "$(slot help)" "Help" "Opens this manual."
row 1052 "$(slot statistics)" "Statistics" "Shows your completed puzzles and their times."
row 1141 "$(slot new)" "New game" "Starts a new puzzle. Refer to page 8."

# The last line of the text is short: it is to the left of the picture of the panel (x 542).
text 72 1276 50 black start "Select colors"
text 72 1328 38 gray25 start "Tap Cell colors to open the panel. It has five sections: starting, clashing"
text 72 1376 38 gray25 start "and highlighted numbers, completed groups and micro-cell lines. In each"
text 72 1424 38 gray25 start "section, tap a color. A black box shows the selected color. A line through"
text 72 1472 38 gray25 start "a color shows that it is in use. White in Completed groups gives no"
text 72 1520 38 gray25 start "completed color."
picture 542 1487 "$(shot "$UI/color-menu.png" $COLOR_PANEL_CROP)"

pagenum 7 9
render "$1/manual-menu.png"
