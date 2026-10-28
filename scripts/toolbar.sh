#!/bin/sh
# Makes the open menu (the toolbar on the right) and its panels in $1. The dimensions are the same as the
# toolbar and the pop-up menus of the device (refer to ui-lib.sh):
#   screen-toolbar.png        menu open on a new game: cell colors, undo and redo (not available), help, analytics, +
#   new-game-menu.png         the + panel is open: the difficulty slider and "Start new game"
#   color-menu.png            the Cell colors panel is open (starting, clashing, highlighted), on a game
#   screen-completed-menu.png menu open on a complete (locked) puzzle: undo and redo not available
#   color-menu-right-handed.png  color-menu.png for a right-handed player (toolbar on the left)
#   settings-menu.png         the Settings panel of a development build
#   screen-toolbar-update.png, settings-update.png, settings-up-to-date.png  updates (stable build v0.1.0)
# Top of the menu: menu button, cell colors, undo, redo. Bottom, from the bottom to the top: + (new game),
# Analytics, Help.
set -e
. "$(dirname "$0")/ui-lib.sh"
OUT=$1
M=$(mktemp)

# The cell background colors in the color panel. They are pale, so that the numbers are easy to read.
COLORS="Gray:#C8C8C8 White:white Yellow:#FFE680 Blue:#AFC4F5 Red:#F0A3A3 Green:#B5DF9E Cyan:#A6E0EE Magenta:#DCB3E6 Orange:#F7C594"
STARTING_BG='#C8C8C8'; CLASH_BG='#F0A3A3'; HIGHLIGHT_BG='#B5DF9E'

# active_slot K : a black square behind toolbar slot K (top slots; "bottom" for the + slot)
active_slot() {
  if [ "$1" = bottom ]; then y=$TB_BOTTOM_Y; else y=$((112*$1)); fi
  echo "stroke none fill black rectangle $TB_X,$y 1620,$((y+111))" >> $M
}

# colors_icon ACTIVE : cell colors (slot 1). A paint bucket: a tilted bucket with a handle and a drop.
colors_icon() {
  cy=$(tb_cy 1); line=black; [ -n "$1" ] && line=white
  echo "push graphic-context translate $TB_CX,$cy fill none stroke $line stroke-width 3 stroke-linecap round stroke-linejoin round" >> $M
  echo "polygon -18,1 -3,-14 12,1 -3,16 line -18,1 12,1 path 'M -10,-7 L -15,-12 C -17,-14 -15,-18 -12,-17 L -5,-12'" >> $M
  echo "stroke none fill $line path 'M 16,3 C 16,3 21,10 21,13 C 21,16 18.8,18 16,18 C 13.2,18 11,16 11,13 C 11,10 16,3 16,3 Z' pop graphic-context" >> $M
}

# gear_icon ACTIVE : settings (slot 2). A gear: a ring, a small center ring and eight teeth.
gear_icon() {
  cy=$(tb_cy 2); line=black; [ -n "$1" ] && line=white
  echo "push graphic-context translate $TB_CX,$cy fill none stroke $line stroke-width 3 circle 0,0 11,0 circle 0,0 4.5,0 stroke-width 6 stroke-linecap butt" >> $M
  for k in 0 1 2 3 4 5 6 7; do
    awk -v k=$k 'BEGIN{ a=k*3.14159265/4; printf "line %.3f,%.3f %.3f,%.3f\n", 11*cos(a), 11*sin(a), 17*cos(a), 17*sin(a) }' >> $M
  done
  echo "pop graphic-context" >> $M
}

# undo_redo COLOR : undo (slot 3) and redo (slot 4). Black when they are available, gray when they are not.
undo_redo() {
  for k in 3 4; do
    cy=$(tb_cy $k); sx=1; [ $k = 4 ] && sx=-1
    echo "push graphic-context translate $TB_CX,$cy scale $sx,1 fill none stroke $1 stroke-width 3 stroke-linecap round stroke-linejoin round" >> $M
    echo "path 'M -18,-4 L 3,-4 C 13,-4 19,4 19,15' polyline -10,-12 -18,-4 -10,4 pop graphic-context" >> $M
  done
}

# plus_icon ACTIVE : + (new game) in the bottom toolbar slot. Analytics and Help are above it.
plus_icon() {
  cy=$((TB_BOTTOM_Y+56)); fg=black
  [ -n "$1" ] && { active_slot bottom; fg=white; }
  echo "fill none stroke $fg stroke-width 3 stroke-linecap round line 1545.5,$cy 1581.5,$cy line $TB_CX,$((cy-18)) $TB_CX,$((cy+18))" >> $M
  # Analytics: a line graph on two axes
  ay=$((TB_BOTTOM_Y-56))
  echo "fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline 1545.5,$((ay-18)) 1545.5,$((ay+18)) 1581.5,$((ay+18))" >> $M
  echo "polyline 1551.5,$((ay+8)) 1559.5,$((ay-2)) 1566.5,$((ay+4)) 1578.5,$((ay-12))" >> $M
  # Help: ? in a circle
  hy=$((TB_BOTTOM_Y-168))
  echo "fill none stroke black stroke-width 3 circle $TB_CX,$hy $((1563+19)).5,$hy" >> $M
  echo "stroke none fill black font '$UI_FONT' font-size 26 text-anchor middle text $TB_CX,$((hy+9)) '?'" >> $M
}

# toolbar UNDO ACTIVE : all menu buttons. UNDO is the color of undo and redo. ACTIVE is "colors", "settings",
# "plus" or "".
toolbar() {
  [ "$2" = colors ] && active_slot 1
  [ "$2" = settings ] && active_slot 2
  colors_icon $([ "$2" = colors ] && echo active); gear_icon $([ "$2" = settings ] && echo active)
  undo_redo $1; plus_icon $([ "$2" = plus ] && echo active)
}

# switch X1 CY ON : a switch of the Settings panel with its right end at X1 (track 72x36, knob of 28 px)
switch() {
  x0=$(($1-72)); y0=$(($2-18)); y1=$(($2+18))
  if [ "$3" = on ]; then
    echo "fill black stroke black stroke-width 3 roundrectangle $x0,$y0 $1,$y1 18,18 stroke none fill white circle $(($1-18)),$2 $(($1-4)),$2" >> $M
  else
    echo "fill white stroke black stroke-width 3 roundrectangle $x0,$y0 $1,$y1 18,18 stroke none fill black circle $((x0+18)),$2 $((x0+32)),$2" >> $M
  fi
}

# panel TOP : an empty pop-up frame from TOP to BOTTOM (default: the screen edge). It touches the toolbar.
PL=1108; PR=1507; TX=$((PL+33)); TR=$((PR-32))
panel() {
  top=$1; bot=${2:-2160}
  echo "stroke none fill white rectangle $PL,$top $PR,$bot" >> $M
  echo "fill black rectangle $PL,$top $PR,$((top+1)) rectangle $PL,$top $((PL+1)),$bot rectangle $((PR-1)),$top $PR,$bot" >> $M
  [ $bot -lt 2160 ] && echo "rectangle $PL,$((bot-1)) $PR,$bot" >> $M
  return 0
}
divider() { echo "stroke none fill black rectangle $PL,$1 $PR,$(($1+1))" >> $M; }
label() { echo "stroke none fill ${4:-black} font '$UI_FONT' font-size $UI_SIZE text-anchor ${3:-start} text $1,$2 '$5'" >> $M; }

# color_section TOP TITLE SELECTED TAKEN... : one 448 px section of the color panel, with the same design as
# the pen-color menu of the device: a title and a 3x3 grid of 112 px cells. A black box shows the selected color.
# The colors of the other two sections (TAKEN) are not available: gray ring, a line through the dot, gray name.
# White is always available, so all sections can use it.
color_section() {
  top=$1; title=$2; sel=$3; shift 3; taken=" $* "
  label $TX $((top+52)) start black "$title"
  i=0
  for c in $COLORS; do
    name=${c%%:*}; col=${c#*:}
    gx=$((PL+32+(i%3)*112)); gy=$((top+79+(i/3)*112)); fg=black; ring=black
    cx=$((gx+56)); cy=$((gy+37))
    if [ "$col" = "$sel" ]; then
      echo "stroke none fill black rectangle $gx,$gy $((gx+111)),$((gy+111))" >> $M; fg=white; ring=white
    elif [ "$col" != white ] && case "$taken" in *" $col "*) true;; *) false;; esac; then
      fg=gray70; ring=gray70
      echo "fill $col stroke $ring stroke-width 2 circle $cx,$cy $((cx+19)),$cy" >> $M
      echo "fill none stroke gray50 stroke-width 2.5 stroke-linecap round line $((cx-17)),$((cy+17)) $((cx+17)),$((cy-17))" >> $M
      label $cx $((gy+88)) middle $fg "$name"; i=$((i+1)); continue
    fi
    echo "fill $col stroke $ring stroke-width 2 circle $cx,$cy $((cx+19)),$cy" >> $M
    label $cx $((gy+88)) middle $fg "$name"
    i=$((i+1))
  done
}

# compose BASE OUTFILE : draw the MVG on BASE
# The menu button shows its open icon (both icons are copies from the device).
# The close button shows at the top left (circle of 38 px, stroke of 3 px, measured on the device).
CLOSE="fill none stroke black stroke-width 3 circle 55.5,55.5 73,55.5 stroke-linecap round line 50,50 61,61 line 61,50 50,61"
compose() { magick "$OUT/$1" "$UI_DIR/icons/ic-toggle.png" -geometry +$TB_X+0 -composite -draw "$CLOSE" -draw @$M -units PixelsPerInch -density 229 "$OUT/$2"; : > $M; }

# --- menu open on a new game
toolbar gray70 ""
compose screen.png screen-toolbar.png

# --- the New game panel opens from the +. It is wider, so that its slider is the same as the meter below the board.
PL=$NG_PL; TX=$((PL+33)); TR=$((TX+DIFF_W))
# One row for the slider (at the last selected mode), then "Start new game" next to the +.
panel $((TB_BOTTOM_Y-112)); divider $TB_BOTTOM_Y
R2=$((TB_BOTTOM_Y-56)); R3=$((TB_BOTTOM_Y+56))
labeled_slider $TX $TR $R2 $LEVEL >> $M; echo >> $M
label $TX $((R3+9)) start black "Start new game"
toolbar gray70 plus
compose screen.png new-game-menu.png
PL=1108; TX=$((PL+33)); TR=$((PR-32))

# --- the Cell colors panel: three sections, on the game in progress (the menu removes the highlight)
panel 112 $((112+3*448))
color_section 112 "Starting numbers" $STARTING_BG "$CLASH_BG" "$HIGHLIGHT_BG"; divider 560
color_section 560 "Clashing numbers" "$CLASH_BG" $STARTING_BG "$HIGHLIGHT_BG"; divider 1008
color_section 1008 "Highlighted numbers" "$HIGHLIGHT_BG" $STARTING_BG "$CLASH_BG"
toolbar black colors
compose screen-in-progress.png color-menu.png

# settings_panel VERSION BOTTOM : the Settings panel: two rows (highlight numbers, remove pencil marks
# automatically), both on, then the version in reMarkable Sans Regular, gray40, at the right. A stable build
# has a fourth row for updates (BOTTOM 681, REQUIREMENTS.md §9.2).
settings_panel() {
  panel 224 $2; divider 338; divider 452
  label $TX $((226+65)) start black "Highlight numbers"; switch $((PR-33)) $((226+56)) on
  label $TX $((340+49)) start black "Remove pencil marks"; label $TX $((340+81)) start black "automatically"; switch $((PR-33)) $((340+56)) on
  label $TX $((454+65)) start black "Version"
  gray_text end $((PR-33)) $((454+65)) "$1"
  [ $2 -gt 567 ] && divider 566
  return 0
}
gray_text() { echo "stroke none fill gray40 font '$DIGIT_FONT' font-size $UI_SIZE text-anchor $1 text $2,$3 '$4'" >> $M; }

# caution : the red caution icon at the top right of the gear (slot 2): its center is 16 px to the right of
# and 16 px above the center of the gear. A red circle with a white border and a white "!".
caution() {
  x=$(awk -v c=$TB_CX 'BEGIN{ printf "%.1f", c+16 }'); y=$(awk -v c=$(tb_cy 2) 'BEGIN{ printf "%.1f", c-16 }')
  awk -v x=$x -v y=$y 'BEGIN{
    printf "fill #D62D20 stroke white stroke-width 3 circle %.1f,%.1f %.1f,%.1f\n", x, y, x+10, y
    printf "fill none stroke white stroke-width 3 stroke-linecap round line %.1f,%.1f %.1f,%.1f\n", x, y-6, x, y+1.5
    printf "stroke none fill white circle %.1f,%.1f %.1f,%.1f\n", x, y+5.5, x+1.6, y+5.5 }' >> $M
}

# update_button TEXT : the update button in the fourth row (top 568): a box with a 2 px border, 33 px from the
# sides of the panel and from 22 px to 90 px below the top of the row, the update icon (a down arrow into a
# tray) 40 px from the left of the box, and the text 72 px from the left of the box.
update_button() {
  bx0=$((PL+33)); bx1=$((PR-33)); by0=$((568+22)); by1=$((568+90)); ix=$((bx0+40)); cy=$((568+56))
  echo "stroke none fill black rectangle $bx0,$by0 $bx1,$((by0+1)) rectangle $bx0,$((by1-1)) $bx1,$by1" >> $M
  echo "rectangle $bx0,$by0 $((bx0+1)),$by1 rectangle $((bx1-1)),$by0 $bx1,$by1" >> $M
  echo "fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round line $ix,$((cy-13)) $ix,$((cy+5))" >> $M
  echo "polyline $((ix-8)),$((cy-3)) $ix,$((cy+5)) $((ix+8)),$((cy-3))" >> $M
  echo "polyline $((ix-13)),$((cy+3)) $((ix-13)),$((cy+13)) $((ix+13)),$((cy+13)) $((ix+13)),$((cy+3))" >> $M
  label $((bx0+72)) $((cy+9)) start black "$1"
}

# --- the Settings panel of a development build ("dev"): no update row
settings_panel dev 567
toolbar black settings
compose screen-in-progress.png settings-menu.png

# --- updates (REQUIREMENTS.md §9.2) for the stable build v0.1.0. v0.2.0 is available: the gear has the
# caution icon, and the update row has the button. Then the same panel when the app is up to date.
toolbar black ""; caution
compose screen-in-progress.png screen-toolbar-update.png
settings_panel v0.1.0 681; update_button "Update to v0.2.0"
toolbar black settings; caution
compose screen-in-progress.png settings-update.png
settings_panel v0.1.0 681; gray_text start $TX $((568+65)) "Sudoku is up to date"
toolbar black settings
compose screen-in-progress.png settings-up-to-date.png

# --- complete puzzle: the board is locked, so undo and redo are not available
toolbar gray70 ""
compose screen-completed.png screen-completed-menu.png
# --- color-menu.png for a right-handed player (REQUIREMENTS.md §3.1). The toolbar moves to the left edge and
# the close button to the top right. The menu icon is a mirror image. The other icons and the panel contents
# do not change: the panel moves 996 px to the left (x 112-511). All moves are full pixels.
magick "$OUT/screen-in-progress.png" \
  \( "$OUT/color-menu.png" -crop 112x2160+$TB_X+0 +repage \) -geometry +0+0 -composite \
  \( "$OUT/color-menu.png" -crop 112x112+$TB_X+0 +repage -flop \) -geometry +0+0 -composite \
  \( "$OUT/color-menu.png" -crop 112x112+0+0 +repage \) -geometry +$TB_X+0 -composite \
  \( "$OUT/color-menu.png" -crop $((PR-PL+1))x$((3*448))+$PL+112 +repage \) -geometry +112+112 -composite \
  -units PixelsPerInch -density 229 "$OUT/color-menu-right-handed.png"
rm -f $M
