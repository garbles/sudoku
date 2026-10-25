#!/bin/sh
# Makes manual-menu.png in $1: page 7 of the manual. It tells about each menu button and how to select a color.
# The icons and the color panel are parts of the UI images. They show at their real size.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "The menu"
text 72 260 38 gray25 start "Tap the menu button in the top right to open the menu."

# row Y ICON HEADING DESCRIPTION : a toolbar icon at its real size, with a heading and one line of description
row() {
  picture 72 $1 "$2"
  text 200 $(($1+42)) 40 black start "$3"
  text 200 $(($1+88)) 34 gray25 start "$4"
}
slot() { shot "$UI/screen-toolbar.png" 112x112+1508+$1; }
slot_on() { shot "$UI/color-menu.png" 112x112+1508+$1; }   # undo and redo are available, as in a game in progress
row 320  "$(slot 0)"    "Menu"                   "Opens and closes the menu."
row 450  "$(slot 112)"  "Cell colors"           "Sets the colors for starting, clashing and highlighted numbers."
row 580  "$(slot_on 224)"  "Undo"                 "Cancels your last change to the board."
row 710  "$(slot_on 336)"  "Redo"                 "Makes the last canceled change again."
row 840  "$(slot 1824)" "Help"                   "Opens this manual."
row 970  "$(slot 1936)" "Analytics"              "Shows your completed puzzles and their times."
row 1100 "$(slot 2048)" "New game"               "Starts a new puzzle. Refer to page 8."

text 72 1320 50 black start "Select colors"
text 72 1390 38 gray25 start "Tap Cell colors to open the"
text 72 1440 38 gray25 start "panel. It has three sections:"
text 72 1490 38 gray25 start "starting, clashing and"
text 72 1540 38 gray25 start "highlighted numbers. In each"
text 72 1590 38 gray25 start "section, tap a color."
text 72 1660 38 gray25 start "A black box shows the selected"
text 72 1710 38 gray25 start "color. A line through a color"
text 72 1760 38 gray25 start "shows that it is in use."
picture 888 1300 "$(shot "$UI/color-menu.png" 512x450+1108+110)"

pagenum 7 9
render "$1/manual-menu.png"
