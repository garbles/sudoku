# UI dimensions of the reMarkable Paper Pro, measured on screenshots of the device (1620x2160).
UI_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_FONT="$UI_DIR/fonts/rmsans-500.ttf"     # reMarkable Sans Medium, the UI font of the system
DIGIT_FONT="$UI_DIR/fonts/rmsans-400.ttf"  # reMarkable Sans Regular, for the board numbers
UI_SIZE=25                                 # size of the menu text
TB_X=1508; TB_W=112; TB_CX=1563.5          # the toolbar column on the right; one item each 112 px
tb_cy() { echo "$1" | awk '{ printf "%.1f", 55.5 + $1*112 }'; }   # the y center of toolbar slot k (from the top)
TB_BOTTOM_Y=2048                           # the top of the bottom toolbar slot
# menu_icon open|closed : the MVG of the icon of the menu button (top toolbar slot), as render.rs `menu_icon`.
# A ring (radius 18 px, line of 3 px) and a dot (radius 4.5 px), 5.5 px to the left of the center and 5.5 px
# above it (open) or below it (closed).
menu_icon() {
  dy=5.5; [ "$1" = open ] && dy=-5.5
  echo "push graphic-context translate $TB_CX,$(tb_cy 0) fill none stroke black stroke-width 3 circle 0,0 18,0" \
    "stroke none fill black circle -5.5,$dy -1,$dy pop graphic-context"
}

# Board: 9x9 cells of 147 px (micro-cells of 49 px), at the center of the screen. With this size, the open menu
# (x>=1508) does not cover the board (gap of 37 px).
C=147; X0=$(((1620-9*C)/2)); Y0=$(((2160-9*C)/2))

# The micro-cell lines of an empty cell (REQUIREMENTS.md §4.1): two vertical and two horizontal lines of 2 px, on the
# pixels MICRO_OFF1, MICRO_OFF1+1, MICRO_OFF2 and MICRO_OFF2+1 from the cell origin, over the full cell. Each pixel of a
# line is LINE_DOT or white, by an ordered 4x4 Bayer dither with the darkness of the selected gray: a pixel is LINE_DOT
# when BAYER[i][j] * 16 + 8 <= 255 - gray, where gray is the level of the setting (gray90 = 230). The row index i is the
# position along the line from its origin (the first pixel of the line), modulo 4, and the column index j is the
# position across the line (0 for the offset pixel, 1 for the pixel after it): a vertical line with its origin at
# (ox, oy) uses BAYER[(y-oy) mod 4][x-ox], a horizontal line uses BAYER[(x-ox) mod 4][y-oy]. The other pixels do not
# change. The swatches of the Micro-cell lines section of the Cell colors panel show the same pattern in a disc, in the
# plain orientation BAYER[(y-oy) mod 4][(x-ox) mod 4] from the top left of the box of the disc (toolbar.sh).
MICRO_OFF1=49; MICRO_OFF2=98
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
NG_PL=$((1507-DIFF_W-65))   # the left edge of the New game panel (inner margins of 33 px and 32 px)
# The New game panel has three rows of 112 px, with a black line of 2 px between them: the slider, "Symmetrical" with a
# switch, and "Start new game" next to the +. Its top is 226 px above the bottom toolbar slot (y 1822).
NG_TOP=$((TB_BOTTOM_Y-226)); NG_SLIDER_Y=$((NG_TOP+56)); NG_SYM_DIVIDER=$((TB_BOTTOM_Y-114)); NG_SYM_Y=$((TB_BOTTOM_Y-56))

# The palette of the cell colors, in panel order. board-state.py and manual-lib.sh have the same values. The digits
# of the player are #1E1E1E. Starting digits are black.
COLOR_GRAY='#AEAEAE'; COLOR_YELLOW='#FFD740'; COLOR_BLUE='#408CFF'; COLOR_RED='#FF8E8E'; COLOR_GREEN='#48D630'
COLOR_CYAN='#6FCFE6'; COLOR_MAGENTA='#E650E0'; COLOR_ORANGE='#FF9A1F'
# The default colors of the four color sections.
STARTING_BG=$COLOR_GRAY; CLASH_BG=$COLOR_RED; HIGHLIGHT_BG=$COLOR_CYAN; COMPLETED_BG=$COLOR_GREEN

# The Settings panel: 400 px wide (x 1108-1507). The rows start at y 226, one each 114 px (112 px and a line of
# 2 px): the three switches, the version, "Third-party licenses", and the update row of a stable build.
SET_PL=1108; SET_PR=1507; SET_TOP=224; SET_ROW0=226; SET_STEP=114
SET_VERSION_Y=$((SET_ROW0+3*SET_STEP)); SET_LICENSES_Y=$((SET_ROW0+4*SET_STEP)); SET_UPDATE_Y=$((SET_ROW0+5*SET_STEP))
SET_BOTTOM_DEV=$((SET_UPDATE_Y-1)); SET_BOTTOM_STABLE=$((SET_UPDATE_Y+SET_STEP-1))   # y 795 and 909

# The Cell colors panel: two columns of 400 px and three rows of 360 px (y 112 to 1192). It touches the toolbar.
# Left column, from the top: starting, clashing, highlighted. Right column: completed, micro-cell lines, empty space.
CM_COL=400; CM_W=$((2*CM_COL)); CM_ROW=360; CM_TOP=112; CM_BOTTOM=$((CM_TOP+3*CM_ROW))
CM_PL=$((TB_X-CM_W))         # 708: left-handed in portrait (x 708-1507)
CM_PL_LANDSCAPE=1248         # left-handed in landscape (x 1248-2047)
CM_PL_RIGHT=$TB_W            # right-handed in the two orientations (x 112-911)

# The Checkpoints panel: 600 px wide, at y 560 in all layouts. It touches the toolbar. It has the row "Save checkpoint"
# (y 560-671), then rows of 180 px, the newest first. The last y of the panel is CKP_EMPTY_BOTTOM with no checkpoint,
# else CKP_ROW0 + 180 n (the bottom line is y-1 and y, as in the other panels).
CKP_W=600; CKP_TOP=560; CKP_ROW0=672; CKP_STEP=180; CKP_EMPTY_BOTTOM=864
CKP_PL=$((TB_X-CKP_W))       # 908: left-handed in portrait (x 908-1507)
CKP_PL_LANDSCAPE=1448        # left-handed in landscape (x 1448-2047)
CKP_PL_RIGHT=$TB_W           # right-handed in the two orientations (x 112-711)
CKP_CX=1207                  # the center of the text of the panel with no checkpoint
CKP_SAVE_ICON_X=966; CKP_SAVE_ICON_Y=616; CKP_SAVE_TEXT_X=1008; CKP_SAVE_BASE=625
# In a row with its top at y0: the small board at (CKP_BOARD_X, y0+CKP_BOARD_DY), the text, the button and the bin.
CKP_BOARD_X=926; CKP_BOARD_DY=14; CKP_CELL=17
CKP_TEXT_X=1098; CKP_CLOCK_DY=81; CKP_CELLS_DY=115
CKP_BTN_X0=1289; CKP_BTN_X1=1429; CKP_BTN_CX=1359; CKP_BTN_Y0=55; CKP_BTN_Y1=125; CKP_BTN_BASE=99
CKP_BIN_X=1468; CKP_BIN_DY=90

# Landscape (2160x1620). The items have the same size as in portrait. Each position is a full-pixel move of the
# portrait position: the board and the meter move by (LS_DX, LS_DY), the toolbar to LS_TB_X, and the bottom toolbar
# slots also by LS_BOTTOM_DY.
LS_W=2160; LS_H=1620; LS_DX=270; LS_DY=-270; LS_TB_X=2048; LS_BOTTOM_DY=-540

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
# Portrait: below the board, the same for the two hands: x 148-1471 (the board width), y 1790-2110.
CARD_X0=$X0; CARD_X1=$((X0+9*C)); CARD_Y0=$((Y0+9*C+49)); CARD_Y1=$((Y0+9*C+49+320))
CARD_MID=$((CARD_X0+640))              # the vertical line between the result and the level buttons (x 788)
# Landscape: a column with the height of the board (y 148-1471), 322 px wide, at the side opposite the toolbar.
CARD_LS_W=322; CARD_LS_X_LEFT=48; CARD_LS_X_RIGHT=$((LS_W-48-CARD_LS_W))   # left-handed x 48-370, right-handed x 1790-2112
CARD_LS_Y0=$((Y0+LS_DY)); CARD_LS_Y1=$((Y0+9*C+LS_DY))
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

# result_card portrait|left|right LEVEL : MVG for the result card. "left" and "right" are the landscape column of a
# left-handed and a right-handed player (2160x1620 coordinates). The contents of the column are the same for the
# two hands. The three level buttons have a white inside (no button is selected). A button that is not available
# has a gray80 frame, a gray70 label and no meter.
result_card() {
  rl=$2
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
