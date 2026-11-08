#!/bin/sh
# Makes manual-finished-landscape.png in $1: page 9 of the manual in landscape (the text of manual-finished.sh).
set -e
LANDSCAPE=1; . "$(dirname "$0")/manual-lib.sh"
M=$(mktemp)
UI=$1

text 72 170 84 black start "Complete a puzzle"

text 72 290 50 black start "Completed"
text 72 355 38 gray25 start "When all cells have a number and there is"
text 72 405 38 gray25 start "no clash, the puzzle is complete. At the side"
text 72 455 38 gray25 start "of the board, the result card then shows your"
text 72 505 38 gray25 start "time and your earlier times at this level."
text 72 555 38 gray25 start "The completed color goes away, and the"
text 72 605 38 gray25 start "board shows the normal colors."
picture_w 72 645 828 "$(shot "$UI/screen-completed.png" $FINISHED_CROP)"

text 972 290 50 black start "The board is locked"
text 972 355 38 gray25 start "You cannot write, erase, add pencil marks"
text 972 405 38 gray25 start "or use undo on a completed puzzle. To start"
text 972 455 38 gray25 start "a new game, tap Easier, Same or Harder on"
text 972 505 38 gray25 start "the result card. The new game starts"
text 972 555 38 gray25 start "immediately."

text 972 680 50 black start "The clock"
text 972 745 38 gray25 start "The clock operates only when the app is open."
text 972 795 38 gray25 start "The app does not count the time when the"
text 972 845 38 gray25 start "app is closed."

text 972 970 50 black start "Your results"
text 972 1035 38 gray25 start "The app saves the difficulty and time of each"
text 972 1085 38 gray25 start "completed puzzle. Analytics in the menu"
text 972 1135 38 gray25 start "shows your totals, average times and a list"
text 972 1185 38 gray25 start "of all games. To delete a game, tap its bin."
text 972 1235 38 gray25 start "The app asks you to confirm."
picture 1690 900 "$(shot "$UI/screen-toolbar.png" 112x112+1508+1936)"

pagenum 9 9
render "$1/manual-finished-landscape.png"
