#!/bin/sh
# Makes manual-menu.png in $1: page 7 of the manual. It tells about each menu button and how to select a color.
# The icons and the color panel are parts of the UI images. They show at their real size.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "The menu"
text 72 245 38 gray25 start "Tap the menu button in the top right to open the menu."

# row Y ICON HEADING DESCRIPTION : a toolbar icon at its real size, with a heading and one line of description
row() {
  picture 72 $1 "$2"
  text 200 $(($1+42)) 40 black start "$3"
  text 200 $(($1+88)) 34 gray25 start "$4"
}
slot() { shot "$UI/screen-toolbar.png" 112x112+1508+$1; }
slot_on() { shot "$UI/color-menu.png" 112x112+1508+$1; }   # undo and redo are available, as in a game in progress
row 270 "$(slot 0)" "Menu" "Opens and closes the menu."
row 368 "$(slot 112)" "Cell colors" "Sets the colors of numbers and completed groups."
row 466 "$(slot 224)" "Settings" "Turns the helpers on or off. Shows the version and the licenses."
row 564 "$(slot_on 336)" "Undo" "Cancels your last change to the board."
row 662 "$(slot_on 448)" "Redo" "Makes the last canceled change again."
row 760 "$(slot 560)" "Checkpoints" "Saves the board. Restore shows it again."
row 858 "$(slot 1824)" "Help" "Opens this manual."
row 956 "$(slot 1936)" "Statistics" "Shows your completed puzzles and their times."
row 1054 "$(slot 2048)" "New game" "Starts a new puzzle. Refer to page 8."

text 72 1205 50 black start "Select colors"
text 72 1260 38 gray25 start "Tap Cell colors to open the panel. It has five sections: starting,"
text 72 1310 38 gray25 start "clashing and highlighted numbers, completed groups and micro-cell"
text 72 1360 38 gray25 start "lines. In each section, tap a color. A black box shows the selected"
text 72 1410 38 gray25 start "color. A line through a color shows that it is in use. White in"
text 72 1460 38 gray25 start "Completed groups gives no completed color."
picture 542 1475 "$(shot "$UI/color-menu.png" $COLOR_PANEL_CROP)"

pagenum 7 9
render "$1/manual-menu.png"
