# UI dimensions, measured on screenshots of the reMarkable Paper Pro (1620x2160). All screen positions come from the
# values of the device (device-lib.sh, devices/$DEVICE.conf) and from the toolbar slot. The numbers in the comments are
# the values on the Paper Pro.
UI_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$UI_DIR/device-lib.sh"
UI_FONT="$UI_DIR/fonts/rmsans-500.ttf"     # reMarkable Sans Medium, the UI font of the system
DIGIT_FONT="$UI_DIR/fonts/rmsans-400.ttf"  # reMarkable Sans Regular, for the board numbers
UI_SIZE=25                                 # size of the menu text
TB_W=$TB_SLOT; TB_X=$((PHYS_W-TB_W))      # the toolbar column on the right (x 1508); one item each 112 px
TB_C=$(awk -v w=$TB_W 'BEGIN{ printf "%.1f", int(w/2) - 0.5 }')   # the center of an icon in its slot (55.5)
TB_CX=$(awk -v x=$TB_X -v c=$TB_C 'BEGIN{ printf "%.1f", x + c }')  # the x center of the toolbar slots (1563.5)
tb_cy() { echo "$1" | awk -v w=$TB_W -v c=$TB_C '{ printf "%.1f", c + $1*w }'; }   # the y center of toolbar slot k (from the top)
TB_BOTTOM_DY=$((TB_W/2))                   # the center of an icon of a bottom slot, below the top of the slot (56)
PANEL_ROW=112                              # the rows of the Settings panel and the row "Save checkpoint", on each model
TB_BOTTOM_Y=$((PHYS_H-TB_W))               # the top of the bottom toolbar slot (y 2048)
# slot_y K : the top of toolbar slot K: 0-5 from the top (menu, cell colors, settings, undo, redo, checkpoints), or
# help, statistics and new (the bottom group: the + is in the bottom slot).
slot_y() {
  case $1 in help) echo $((TB_BOTTOM_Y-2*TB_W)) ;; statistics) echo $((TB_BOTTOM_Y-TB_W)) ;; new) echo $TB_BOTTOM_Y ;; *) echo $(($1*TB_W)) ;; esac
}
# slot_crop K : the ImageMagick crop of toolbar slot K (refer to slot_y)
slot_crop() { echo "${TB_W}x$TB_W+$TB_X+$(slot_y $1)"; }
# menu_icon open|closed : the MVG of the icon of the menu button (top toolbar slot), as render.rs `menu_icon`.
# A ring (radius 18 px, line of 3 px) and a dot (radius 4.5 px), 5.5 px to the left of the center and 5.5 px
# above it (open) or below it (closed).
menu_icon() {
  dy=5.5; [ "$1" = open ] && dy=-5.5
  echo "push graphic-context translate $TB_CX,$(tb_cy 0) fill none stroke black stroke-width 3 circle 0,0 18,0" \
    "stroke none fill black circle -5.5,$dy -1,$dy pop graphic-context"
}

# Board: 9x9 cells of C px (147 px, micro-cells of 49 px), at the center of the screen. On the Paper Pro, the open menu
# (x>=1508) does not cover the board (gap of 37 px).
# Each board line has its own pixels between the cells (src/layout.rs line_offset): the border and the box lines are
# BOX_LINE px, the lines between the cells CELL_LINE px. BOARD is the side of the board (1369 px on the Paper Pro), and
# X0, Y0 its top left (the first pixel of the border).
BOX_LINE=7; CELL_LINE=3
BOARD=$((9*C+4*BOX_LINE+6*CELL_LINE))
X0=$(((PHYS_W-BOARD)/2)); Y0=$(((PHYS_H-BOARD)/2))
# line_w K, line_off K, cell_off I : the width of board line K (0..9), its first pixel from the board edge, and the
# first pixel of the cells of column (or row) I (after line I)
line_w() { if [ $(($1%3)) = 0 ]; then echo $BOX_LINE; else echo $CELL_LINE; fi; }
line_off() { echo $(($1*C+($1+2)/3*BOX_LINE+($1-($1+2)/3)*CELL_LINE)); }
cell_off() { echo $(($(line_off $1)+$(line_w $1))); }

# The micro-cell lines of an empty cell (REQUIREMENTS.md §4.1): two vertical and two horizontal lines of 2 px, on the
# pixels MICRO_OFF1, MICRO_OFF1+1, MICRO_OFF2 and MICRO_OFF2+1 from the cell origin, over the full cell. Each pixel of a
# line is LINE_DOT or white, by an ordered 4x4 Bayer dither with the darkness of the selected gray: a pixel is LINE_DOT
# when BAYER[i][j] * 16 + 8 <= 255 - gray, where gray is the level of the setting (gray90 = 230). The row index i is the
# position along the line from its origin (the first pixel of the line), modulo 4, and the column index j is the
# position across the line (0 for the offset pixel, 1 for the pixel after it): a vertical line with its origin at
# (ox, oy) uses BAYER[(y-oy) mod 4][x-ox], a horizontal line uses BAYER[(x-ox) mod 4][y-oy]. The other pixels do not
# change. The swatches of the Micro-cell lines section of the Cell colors panel show the same pattern in a disc, in the
# plain orientation BAYER[(y-oy) mod 4][(x-ox) mod 4] from the top left of the box of the disc (toolbar.sh).
MICRO_OFF1=$(((C+1)/3)); MICRO_OFF2=$(((2*C+1)/3))   # round(k * C / 3), as layout::micro_offset (49 and 98)
BAYER="0 8 2 10 12 4 14 6 3 11 1 9 15 7 13 5"
LINE_DOT=gray15
# gray_level grayNN : the level (0-255) of the gray, as the value of the setting (gray90 = 230, gray40 = 102)
gray_level() { echo "${1#gray}" | awk '{ printf "%d", $1*255/100+0.5 }'; }
# bayer_points GRAY OX OY XA YA XB YB AXIS [CX CY R] : the MVG points (fill LINE_DOT, one pixel each, no anti-aliasing)
# of the pattern of the gray level GRAY (0-255) with its origin at (OX, OY), in the pixels XA..XB x YA..YB. AXIS is the
# axis of the row index of the matrix: y (the plain orientation, a vertical line, the disc) or x (a horizontal line).
# With CX CY R, only the pixels inside the disc: (x-cx)^2 + (y-cy)^2 < r^2, with x and y the integer pixel coordinates.
bayer_points() {
  awk -v g=$1 -v ox=$2 -v oy=$3 -v xa=$4 -v ya=$5 -v xb=$6 -v yb=$7 -v axis=$8 -v cx=$9 -v cy=${10} -v r=${11} \
      -v m="$BAYER" -v dot=$LINE_DOT 'BEGIN{
    split(m, b, " "); s="stroke none fill " dot
    for (y=ya;y<=yb;y++) for (x=xa;x<=xb;x++) {
      if (r!="" && (x-cx)^2+(y-cy)^2 >= r*r) continue
      dy=((y-oy)%4+4)%4; dx=((x-ox)%4+4)%4
      if (axis=="x") { i=dx; j=dy } else { i=dy; j=dx }
      if (b[i*4+j+1]*16+8 <= 255-g) s=s sprintf(" point %d,%d", x, y) }
    print s }'
}

# Seven difficulty modes (0-6). LEVEL is the mode of the current puzzle. The New game panel opens at the last
# selected mode. At the first start, this is mode 2. The generator sets the techniques that each mode needs.
STOPS=7
LEVEL=2

# Difficulty meter: "Easy" to "Hard" is DIFF_W px wide. It is the same in the New game panel and below the board.
# Thus the New game panel is wider than 400 px (the width of the device menus).
DIFF_W=440
NG_PL=$((TB_X-1-DIFF_W-65))   # the left edge of the New game panel (inner margins of 33 px and 32 px; x 1002)
# The New game panel has three rows of 112 px, with a black line of 2 px between them: the slider, "Symmetrical" with a
# switch, and "Start new game" next to the +. Its top is 226 px above the bottom toolbar slot (y 1822).
NG_TOP=$((TB_BOTTOM_Y-2*TB_W-2)); NG_SLIDER_Y=$((NG_TOP+TB_W/2)); NG_SYM_DIVIDER=$((TB_BOTTOM_Y-TB_W-2)); NG_SYM_Y=$((TB_BOTTOM_Y-TB_W/2))

# The palette of the cell colors, in panel order. board-state.py and manual-lib.sh have the same values. The digits
# of the player are #1E1E1E. Starting digits are black.
COLOR_GRAY='#AEAEAE'; COLOR_YELLOW='#FFD740'; COLOR_BLUE='#408CFF'; COLOR_RED='#FF8E8E'; COLOR_GREEN='#48D630'
COLOR_CYAN='#6FCFE6'; COLOR_MAGENTA='#E650E0'; COLOR_ORANGE='#FF9A1F'
# The default colors of the four color sections.
STARTING_BG=$COLOR_GRAY; CLASH_BG=$COLOR_RED; HIGHLIGHT_BG=$COLOR_CYAN; COMPLETED_BG=$COLOR_GREEN

# The Settings panel: 400 px wide (x 1108-1507), from the top of the slot of the gear (slot 2). The rows start at y 226, one each 114 px (112 px and a line of
# 2 px): the three switches, the version, "Third-party licenses", and the update row of a stable build.
SET_W=400; SET_PL=$((TB_X-SET_W)); SET_PR=$((TB_X-1)); SET_TOP=$((2*TB_W)); SET_ROW0=$((SET_TOP+2)); SET_STEP=$((PANEL_ROW+2))
SET_VERSION_Y=$((SET_ROW0+3*SET_STEP)); SET_LICENSES_Y=$((SET_ROW0+4*SET_STEP)); SET_UPDATE_Y=$((SET_ROW0+5*SET_STEP))
SET_BOTTOM_DEV=$((SET_UPDATE_Y-1)); SET_BOTTOM_STABLE=$((SET_UPDATE_Y+SET_STEP-1))   # y 795 and 909

# Landscape (2160x1620). The items have the same size as in portrait. Each position is a full-pixel move of the
# portrait position: the board and the meter move by (LS_DX, LS_DY) (270, -270), the toolbar to LS_TB_X (2048), and
# the bottom toolbar slots also by LS_BOTTOM_DY (-540).
LS_W=$PHYS_H; LS_H=$PHYS_W
LS_DX=$(((LS_W-BOARD)/2-X0)); LS_DY=$(((LS_H-BOARD)/2-Y0)); LS_TB_X=$((LS_W-TB_W)); LS_BOTTOM_DY=$((LS_H-PHYS_H))

# The Cell colors panel: two columns of 400 px and three rows of 360 px (y 112 to 1192). It touches the toolbar.
# Left column, from the top: starting, clashing, highlighted. Right column: completed, micro-cell lines, empty space.
# The panel has three rows when they fit on the screen, else two rows and three columns (the Move in landscape,
# src/layout.rs color_rows). CM_* is portrait; CM_*_LANDSCAPE is landscape.
CM_COL=400; CM_ROW=360; CM_TOP=$TB_W   # from slot 1
cm_rows() { if [ $((CM_TOP+3*CM_ROW)) -lt $1 ]; then echo 3; else echo 2; fi; }   # cm_rows SCREEN_H
CM_ROWS=$(cm_rows $PHYS_H); CM_W=$((6/CM_ROWS*CM_COL)); CM_BOTTOM=$((CM_TOP+CM_ROWS*CM_ROW))
CM_PL=$((TB_X-CM_W))         # 708: left-handed in portrait (x 708-1507)
CM_ROWS_LANDSCAPE=$(cm_rows $LS_H); CM_W_LANDSCAPE=$((6/CM_ROWS_LANDSCAPE*CM_COL))
CM_BOTTOM_LANDSCAPE=$((CM_TOP+CM_ROWS_LANDSCAPE*CM_ROW))
CM_PL_LANDSCAPE=$((LS_TB_X-CM_W_LANDSCAPE))   # 1248: left-handed in landscape (x 1248-2047)
CM_PL_RIGHT=$TB_W            # right-handed in the two orientations (x 112-911)

# The Checkpoints panel: 600 px wide, at y 560 in all layouts. It touches the toolbar. It has the row "Save checkpoint"
# (y 560-671), then rows of 180 px, the newest first. The last y of the panel is CKP_EMPTY_BOTTOM with no checkpoint,
# else CKP_ROW0 + 180 n (the bottom line is y-1 and y, as in the other panels).
# The panel starts at the slot of the bookmark (slot 5). If the panel with the maximum of 4 rows does not fit below
# it, it starts at slot 1 (the Move in landscape, src/layout.rs ckp_top). CKP_TOP_LANDSCAPE is the top in landscape.
CKP_W=600; CKP_STEP=180; CKP_MAX=4
ckp_top() { if [ $((5*TB_W+PANEL_ROW+CKP_MAX*CKP_STEP)) -lt $1 ]; then echo $((5*TB_W)); else echo $TB_W; fi; }   # ckp_top SCREEN_H
CKP_TOP=$(ckp_top $PHYS_H); CKP_TOP_LANDSCAPE=$(ckp_top $LS_H)
CKP_ROW0=$((CKP_TOP+PANEL_ROW)); CKP_EMPTY_BOTTOM=$((CKP_TOP+304))
CKP_PL=$((TB_X-CKP_W))       # 908: left-handed in portrait (x 908-1507)
CKP_PL_LANDSCAPE=$((LS_TB_X-CKP_W))   # 1448: left-handed in landscape (x 1448-2047)
CKP_PL_RIGHT=$TB_W           # right-handed in the two orientations (x 112-711)
# The positions in the panel are from its left edge (CKP_PL) and its top (CKP_TOP), as in src/layout.rs.
CKP_CX=$((CKP_PL+299))       # 1207: the center of the text of the panel with no checkpoint
CKP_EMPTY_BASE1=$((CKP_TOP+200)); CKP_EMPTY_BASE2=$((CKP_TOP+240))   # y 760 and 800: the two lines of that text
CKP_SAVE_ICON_X=$((CKP_PL+58)); CKP_SAVE_ICON_Y=$((CKP_TOP+56)); CKP_SAVE_TEXT_X=$((CKP_PL+100)); CKP_SAVE_BASE=$((CKP_TOP+65))
# In a row with its top at y0: the small board at (CKP_BOARD_X, y0+CKP_BOARD_DY), the text, the button and the bin.
CKP_BOARD_X=$((CKP_PL+18)); CKP_BOARD_DY=14; CKP_CELL=17
CKP_TEXT_X=$((CKP_PL+190)); CKP_CLOCK_DY=81; CKP_CELLS_DY=115
CKP_BTN_X0=$((CKP_PL+381)); CKP_BTN_X1=$((CKP_PL+521)); CKP_BTN_CX=$((CKP_PL+451)); CKP_BTN_Y0=55; CKP_BTN_Y1=125; CKP_BTN_BASE=99
CKP_BIN_X=$((CKP_PL+560)); CKP_BIN_DY=90

# the text width of a UI label, in px
tw() { magick -font "$UI_FONT" -pointsize $UI_SIZE label:"$1" -format %w info:; }

# slider XA XB Y LEVEL : MVG for a difficulty slider with STOPS stops, from XA to XB
slider() {
  awk -v xa=$1 -v xb=$2 -v y=$3 -v L=$4 -v n=$STOPS 'BEGIN{
    step=(xb-xa)/(n-1); xk=xa+L*step
    printf "fill none stroke-linecap round stroke gray60 stroke-width 3 line %.1f,%d %.1f,%d ", xk, y, xb, y
    printf "stroke black stroke-width 5 line %.1f,%d %.1f,%d ", xa, y, xk, y
    for (i=0;i<n;i++) { x=xa+i*step; printf "fill %s stroke black stroke-width 3 circle %.1f,%d %.1f,%d ", (i<L?"black":"white"), x, y, x+6, y }
    printf "fill black stroke white stroke-width 3 circle %.1f,%d %.1f,%d", xk, y, xk+13, y }'
}

# labeled_slider XL XR Y LEVEL : "Easy" at XL, the slider, and "Hard" that ends at XR (baseline = Y+9, as on the device)
labeled_slider() {
  we=$(tw Easy); wi=$(tw Hard)
  printf "stroke none fill black font '%s' font-size %s text-anchor start text %s,%s 'Easy' " "$UI_FONT" $UI_SIZE $1 $(($3+9))
  printf "text-anchor end text %s,%s 'Hard' " $2 $(($3+9))
  slider $(($1+we+24)) $(($2-wi-24)) $3 $4
}

# The secondary text gray (REQUIREMENTS.md §15): gray25 (#404040). The old gray40 was too light on the device.
TEXT_GRAY=gray25

# meter XA Y STEP LEVEL : MVG for the small difficulty meter with no labels (render.rs `meter`, the meter of the
# statistics tables), in screen pixels. Its first stop is at XA, and its stops are STEP px apart.
meter() {
  awk -v xa=$1 -v y=$2 -v s=$3 -v L=$4 -v n=$STOPS 'BEGIN{
    xk=xa+L*s; xb=xa+(n-1)*s
    printf "fill none stroke-linecap round stroke gray60 stroke-width 2.5 line %g,%g %g,%g ", xk, y, xb, y
    printf "stroke black stroke-width 4.5 line %g,%g %g,%g ", xa, y, xk, y
    for (i=0;i<n;i++) { x=xa+i*s; printf "fill %s stroke black stroke-width 2.5 circle %g,%g %g,%g ", (i<L?"black":"white"), x, y, x+5, y }
    printf "fill black stroke white stroke-width 2.5 circle %g,%g %g,%g", xk, y, xk+11, y }'
}

# The result card of a completed puzzle (REQUIREMENTS.md §8.3). It replaces the difficulty meter.
# Portrait: below the board, the same for the two hands: x 125-1494 (the board width), y 1813-2133.
CARD_X0=$X0; CARD_X1=$((X0+BOARD)); CARD_Y0=$((Y0+BOARD+49)); CARD_Y1=$((Y0+BOARD+49+320))
# The compact card of a small screen (COMPACT_CARD=1, src/layout.rs result_card): in portrait it is 370 px high, at the
# center of the screen and 24 px from the toolbar. In landscape it is the same column, with smaller spaces.
if [ "$COMPACT_CARD" = 1 ]; then
  CARD_HALF=$((PHYS_W/2-TB_W-24)); CARD_X0=$((PHYS_W/2-CARD_HALF)); CARD_X1=$((PHYS_W/2+CARD_HALF)); CARD_Y1=$((CARD_Y0+370))
fi
# The landscape column of the compact card is 300 px wide, one toolbar slot and 24 px from the screen edge (below).
CARD_MID=$((CARD_X0+640))              # the vertical line between the result and the level buttons (x 788)
# Landscape: a column with the height of the board (y 148-1471), 322 px wide, at the side opposite the toolbar.
CARD_LS_W=322; CARD_LS_X_LEFT=48; CARD_LS_X_RIGHT=$((LS_W-48-CARD_LS_W))   # left-handed x 48-370, right-handed x 1790-2112
if [ "$COMPACT_CARD" = 1 ]; then CARD_LS_W=300; CARD_LS_X_LEFT=$((TB_W+24)); CARD_LS_X_RIGHT=$((LS_W-TB_W-24-CARD_LS_W)); fi
CARD_LS_Y0=$((Y0+LS_DY)); CARD_LS_Y1=$((Y0+BOARD+LS_DY))
CARD_LS_SPLIT=560                      # the horizontal line is 560 px above the bottom of the column (y 911)
# The sample values of the reference state: the time, and the comparison with the results of analytics.py at
# mode 2 (LEVEL), without the result of the current game (seed 1, the second result).
CARD_TIME="23:41"; CARD_AVERAGE="18:55"; CARD_BEST="14:51"

# card_text FONT SIZE COLOR ANCHOR X Y TEXT
card_text() { printf "stroke none fill %s font '%s' font-size %s text-anchor %s text %s,%s '%s'\n" "$3" "$1" $2 $4 $5 $6 "$7"; }
# card_frame X0 Y0 X1 Y1 COLOR : a frame of 2 px on the lines of the rectangle (columns X0-1, X0 and X1-1, X1; the same
# for the rows), as a stroke of 2 px of render.rs `stroke_rect`
card_frame() {
  printf "stroke none fill %s rectangle %s,%s %s,%s rectangle %s,%s %s,%s " $5 $(($1-1)) $(($2-1)) $3 $2 $(($1-1)) $(($4-1)) $3 $4
  printf "rectangle %s,%s %s,%s rectangle %s,%s %s,%s\n" $(($1-1)) $(($2-1)) $1 $4 $(($3-1)) $(($2-1)) $3 $4
}
# level_label I : the label of level button I (0 Easier, 1 Same, 2 Harder)
level_label() { case $1 in 0) echo Easier ;; 1) echo Same ;; *) echo Harder ;; esac; }
# level_mode I LEVEL : the mode that level button I starts, or nothing when the button is not available
level_mode() { lm=$(($2+$1-1)); if [ $lm -ge 0 ] && [ $lm -lt $STOPS ]; then echo $lm; fi; }

# compact_card portrait|left|right LEVEL : MVG for the compact result card (src/render.rs draw_compact_card_below and draw_card_column). Portrait:
# the result at the top, a horizontal gray70 line 250 px below the top, then the three level buttons in one row (80 px
# high, 32 px from the sides, 16 px between them), each with its label above a small meter. Landscape: the column of
# result_card with smaller spaces, the line 380 px below the top and buttons of 88 px with a pitch of 100 px (no
# "New game" heading).
compact_card() {
  rl=$2
  if [ $1 = portrait ]; then
    rx0=$CARD_X0; ry0=$CARD_Y0; rx1=$CARD_X1; ry1=$CARD_Y1; rtx=$((rx0+40)); div=$((ry0+250))
    echo "stroke none fill white rectangle $((rx0-1)),$((ry0-1)) $rx1,$ry1"
    card_frame $rx0 $ry0 $rx1 $ry1 black
    card_text "$DIGIT_FONT" 30 $TEXT_GRAY start $rtx $((ry0+50)) "Completed in"
    card_text "$UI_FONT" 72 black start $rtx $((ry0+130)) "$CARD_TIME"
    labeled_slider $rtx $((rtx+DIFF_W)) $((ry0+175)) $rl; echo
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+225)) "Average $CARD_AVERAGE  ·  Previous best $CARD_BEST"
    echo "stroke none fill gray70 rectangle $((rx0+24)),$((div-1)) $((rx1-25)),$div"
    bw=$(((rx1-rx0-64-32)/3))
    for i in 0 1 2; do
      bx0=$((rx0+32+i*(bw+16))); bx1=$((bx0+bw)); by0=$((div+18)); by1=$((by0+80)); bcx=$(((bx0+bx1)/2))
      bm=$(level_mode $i $rl); fg=black; [ -z "$bm" ] && fg=gray70
      card_frame $bx0 $by0 $bx1 $by1 $([ -n "$bm" ] && echo black || echo gray80)
      card_text "$UI_FONT" 28 $fg middle $bcx $((by0+34)) "$(level_label $i)"
      [ -n "$bm" ] && { meter $((bcx-3*22)) $((by0+58)) 22 $bm; echo; }
    done
  else
    rx0=$CARD_LS_X_LEFT; [ $1 = right ] && rx0=$CARD_LS_X_RIGHT
    rx1=$((rx0+CARD_LS_W)); ry0=$CARD_LS_Y0; ry1=$CARD_LS_Y1; rtx=$((rx0+28)); rcx=$(((rx0+rx1)/2)); div=$((ry0+380))
    card_frame $rx0 $ry0 $rx1 $ry1 black
    card_text "$DIGIT_FONT" 30 $TEXT_GRAY start $rtx $((ry0+50)) "Completed in"
    card_text "$UI_FONT" 64 black start $rtx $((ry0+128)) "$CARD_TIME"
    meter $((rcx-3*36)) $((ry0+176)) 36 $rl; echo
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+232)) "Average at this level"
    card_text "$UI_FONT" 36 black start $rtx $((ry0+272)) "$CARD_AVERAGE"
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+312)) "Previous best"
    card_text "$UI_FONT" 36 black start $rtx $((ry0+352)) "$CARD_BEST"
    echo "stroke none fill gray70 rectangle $rtx,$((div-1)) $((rx1-29)),$div"
    for i in 0 1 2; do
      bx0=$((rx0+20)); bx1=$((rx1-20)); by0=$((div+20+100*i)); by1=$((by0+88))
      bm=$(level_mode $i $rl); fg=black; [ -z "$bm" ] && fg=gray70
      card_frame $bx0 $by0 $bx1 $by1 $([ -n "$bm" ] && echo black || echo gray80)
      card_text "$UI_FONT" 30 $fg middle $rcx $((by0+38)) "$(level_label $i)"
      [ -n "$bm" ] && { meter $((rcx-3*30)) $((by0+66)) 30 $bm; echo; }
    done
  fi
}

# result_card portrait|left|right LEVEL : MVG for the result card. "left" and "right" are the landscape column of a
# left-handed and a right-handed player (2160x1620 coordinates). The contents of the column are the same for the
# two hands. The three level buttons have a white inside (no button is selected). A button that is not available
# has a gray80 frame, a gray70 label and no meter.
result_card() {
  rl=$2
  if [ "$COMPACT_CARD" = 1 ]; then compact_card $1 $2; return 0; fi
  if [ $1 = portrait ]; then
    rx0=$CARD_X0; ry0=$CARD_Y0; rx1=$CARD_X1; ry1=$CARD_Y1; rtx=$((rx0+40))
    echo "stroke none fill white rectangle $((rx0-1)),$((ry0-1)) $rx1,$ry1"
    card_frame $rx0 $ry0 $rx1 $ry1 black
    card_text "$DIGIT_FONT" 30 $TEXT_GRAY start $rtx $((ry0+66)) "Completed in"
    card_text "$UI_FONT" 96 black start $rtx $((ry0+180)) "$CARD_TIME"
    labeled_slider $rtx $((rtx+DIFF_W)) $((ry0+232)) $rl; echo
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+293)) "Average $CARD_AVERAGE  ·  Previous best $CARD_BEST"
    echo "stroke none fill gray70 rectangle $((CARD_MID-1)),$((ry0+24)) $CARD_MID,$((ry1-25))"
    card_text "$UI_FONT" 28 black start $((CARD_MID+32)) $((ry0+55)) "New game"
    for i in 0 1 2; do
      bx0=$((CARD_MID+32)); bx1=$((rx1-32)); by0=$((ry0+75+80*i)); by1=$((by0+70)); bcy=$(((by0+by1)/2))
      bm=$(level_mode $i $rl); fg=black; [ -z "$bm" ] && fg=gray70
      card_frame $bx0 $by0 $bx1 $by1 $([ -n "$bm" ] && echo black || echo gray80)
      card_text "$UI_FONT" 30 $fg start $((bx0+28)) $((bcy+11)) "$(level_label $i)"
      [ -n "$bm" ] && { meter $((bx1-40-6*30)) $bcy 30 $bm; echo; }
    done
  else
    rx0=$CARD_LS_X_LEFT; [ $1 = right ] && rx0=$CARD_LS_X_RIGHT
    rx1=$((rx0+CARD_LS_W)); ry0=$CARD_LS_Y0; ry1=$CARD_LS_Y1; rtx=$((rx0+28)); rcx=$(((rx0+rx1)/2)); sy=$((ry1-CARD_LS_SPLIT))
    card_frame $rx0 $ry0 $rx1 $ry1 black
    card_text "$DIGIT_FONT" 30 $TEXT_GRAY start $rtx $((ry0+70)) "Completed in"
    card_text "$UI_FONT" 72 black start $rtx $((ry0+185)) "$CARD_TIME"
    meter $((rcx-3*40)) $((ry0+255)) 40 $rl; echo
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+370)) "Average at this level"
    card_text "$UI_FONT" 40 black start $rtx $((ry0+418)) "$CARD_AVERAGE"
    card_text "$DIGIT_FONT" 25 $TEXT_GRAY start $rtx $((ry0+480)) "Previous best"
    card_text "$UI_FONT" 40 black start $rtx $((ry0+528)) "$CARD_BEST"
    echo "stroke none fill gray70 rectangle $rtx,$((sy-1)) $((rx1-29)),$sy"
    card_text "$UI_FONT" 28 black start $rtx $((sy+60)) "New game"
    for i in 0 1 2; do
      bx0=$((rx0+20)); bx1=$((rx1-20)); by0=$((sy+90+140*i)); by1=$((by0+120))
      bm=$(level_mode $i $rl); fg=black; [ -z "$bm" ] && fg=gray70
      card_frame $bx0 $by0 $bx1 $by1 $([ -n "$bm" ] && echo black || echo gray80)
      card_text "$UI_FONT" 32 $fg middle $rcx $((by0+50)) "$(level_label $i)"
      [ -n "$bm" ] && { meter $((rcx-3*36)) $((by0+88)) 36 $bm; echo; }
    done
  fi
  return 0
}
