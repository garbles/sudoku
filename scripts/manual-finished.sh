#!/bin/sh
# Makes manual-finished.png in $1: page 9 of the manual. It tells about completion, the result card, the lock, the clock and the results.
set -e
. "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "Complete a puzzle"

text 72 290 50 black start "Completed"
text 72 355 38 gray25 start "When all cells have a number and there is no clash, the puzzle"
text 72 405 38 gray25 start "is complete. Below the board, the result card then shows your"
text 72 455 38 gray25 start "time and your earlier times at this level. The completed color"
text 72 505 38 gray25 start "goes away, and the board shows the normal colors of the numbers."
picture 123 540 "$(shot "$UI/screen-completed.png" $FINISHED_CROP)"

text 72 1065 50 black start "The board is locked"
text 72 1130 38 gray25 start "You cannot write, erase, add pencil marks or use undo on"
text 72 1180 38 gray25 start "a completed puzzle. To start a new game, tap Easier, Same or"
text 72 1230 38 gray25 start "Harder on the result card. The new game starts immediately."

text 72 1330 50 black start "The clock"
text 72 1395 38 gray25 start "The clock operates only when the app is open. The app does"
text 72 1445 38 gray25 start "not count the time when the app is closed."

text 72 1540 50 black start "Your results"
text 72 1605 38 gray25 start "The app saves the difficulty and time of each completed puzzle."
text 72 1655 38 gray25 start "Analytics in the menu shows your totals, average times"
text 72 1705 38 gray25 start "and a list of all games. To delete a game, tap its bin."
text 72 1755 38 gray25 start "The app asks you to confirm."
picture 1235 1480 "$(shot "$UI/screen-toolbar.png" 112x112+1508+1936)"

pagenum 9 9
render "$1/manual-finished.png"
