#!/bin/sh
# Makes manual-finished-landscape.png in $1: page 9 of the manual in landscape (the text of manual-finished.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "Complete a puzzle"

text 72 290 50 black start "Completed"
text 72 355 38 gray25 start "When all cells have a number and there is"
text 72 405 38 gray25 start "no clash, the puzzle is complete. Below the"
text 72 455 38 gray25 start "board, the app then shows the time that you"
text 72 505 38 gray25 start "used."
picture_w 72 550 828 "$(shot "$UI/screen-completed.png" 1336x380+142+1441)"

text 72 880 50 black start "The board is locked"
text 72 945 38 gray25 start "You cannot write, erase, add pencil marks"
text 72 995 38 gray25 start "or use undo on a completed puzzle. To start"
text 72 1045 38 gray25 start "a new game, tap + in the menu."

text 972 290 50 black start "The clock"
text 972 355 38 gray25 start "The clock operates only when the app is open."
text 972 405 38 gray25 start "The app does not count the time when the"
text 972 455 38 gray25 start "app is closed."

text 972 580 50 black start "Your results"
text 972 645 38 gray25 start "The app saves the difficulty and time of each"
text 972 695 38 gray25 start "completed puzzle. Analytics in the menu"
text 972 745 38 gray25 start "shows your totals, average times and a list"
text 972 795 38 gray25 start "of all games. To delete a game, tap its bin."
text 972 845 38 gray25 start "The app asks you to confirm."
picture 1690 510 "$(shot "$UI/screen-toolbar.png" 112x112+1508+1936)"

pagenum 9 9
render "$1/manual-finished-landscape.png"
