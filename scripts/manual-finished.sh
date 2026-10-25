#!/bin/sh
# Makes manual-finished.png in $1: page 9 of the manual. It tells about completion, the lock, the clock and the results.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "Complete a puzzle"

text 72 290 50 black start "Completed"
text 72 355 38 gray25 start "When all cells have a number and there is no clash, the puzzle"
text 72 405 38 gray25 start "is complete. Below the board, the app then shows the time"
text 72 455 38 gray25 start "that you used."
picture 123 500 "$(shot "$UI/screen-completed.png" 1336x380+142+1441)"

text 72 930 50 black start "The board is locked"
text 72 995 38 gray25 start "You cannot write, erase, add pencil marks or use undo on"
text 72 1045 38 gray25 start "a completed puzzle. To start a new game, tap + in the menu."

text 72 1200 50 black start "The clock"
text 72 1265 38 gray25 start "The clock operates only when the app is open. The app does"
text 72 1315 38 gray25 start "not count the time when the app is closed."

text 72 1420 50 black start "Your results"
text 72 1485 38 gray25 start "The app saves the difficulty and time of each completed puzzle."
text 72 1535 38 gray25 start "Analytics in the menu shows your totals, average times"
text 72 1585 38 gray25 start "and a list of all games. To delete a game, tap its bin."
text 72 1635 38 gray25 start "The app asks you to confirm."
picture 1235 1360 "$(shot "$UI/screen-toolbar.png" 112x112+1508+1936)"

pagenum 9 9
render "$1/manual-finished.png"
