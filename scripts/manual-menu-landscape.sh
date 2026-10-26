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
slot() { shot "$UI/screen-toolbar.png" 112x112+1508+$1; }
slot_on() { shot "$UI/color-menu.png" 112x112+1508+$1; }
row 300  "$(slot 0)" "Menu"                   "Opens and closes the menu."
row 420  "$(slot 112)" "Cell colors"            "Sets the colors for starting, clashing and highlighted numbers."
row 540  "$(slot 224)" "Settings"               "Turns highlights and automatic pencil mark removal on or off."
row 660  "$(slot_on 336)" "Undo"                   "Cancels your last change to the board."
row 780  "$(slot_on 448)" "Redo"                   "Makes the last canceled change again."
row 900  "$(slot 1824)" "Help"                   "Opens this manual."
row 1020 "$(slot 1936)" "Statistics"             "Shows your completed puzzles and their times."
row 1140 "$(slot 2048)" "New game"               "Starts a new puzzle. Refer to page 8."

text 1330 340 50 black start "Select colors"
text 1330 405 38 gray25 start "Tap Cell colors to open the"
text 1330 455 38 gray25 start "panel. It has three sections:"
text 1330 505 38 gray25 start "starting, clashing and"
text 1330 555 38 gray25 start "highlighted numbers. In each"
text 1330 605 38 gray25 start "section, tap a color."
text 1330 675 38 gray25 start "A black box shows the selected"
text 1330 725 38 gray25 start "color. A line through a color"
text 1330 775 38 gray25 start "shows that it is in use."
picture 1356 820 "$(shot "$UI/color-menu.png" 512x450+1108+110)"

pagenum 7 9
render "$1/manual-menu-landscape.png"
