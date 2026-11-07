# UI dimensions of the reMarkable Paper Pro, measured on screenshots of the device (1620x2160).
UI_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_FONT="$UI_DIR/fonts/rmsans-500.ttf"     # reMarkable Sans Medium, the UI font of the system
DIGIT_FONT="$UI_DIR/fonts/rmsans-400.ttf"  # reMarkable Sans Regular, for the board numbers
UI_SIZE=25                                 # size of the menu text
TB_X=1508; TB_W=112; TB_CX=1563.5          # the toolbar column on the right; one item each 112 px
tb_cy() { echo "$1" | awk '{ printf "%.1f", 55.5 + $1*112 }'; }   # the y center of toolbar slot k (from the top)
TB_BOTTOM_Y=2048                           # the top of the bottom toolbar slot

# Board: 9x9 cells of 147 px (micro-cells of 49 px), at the center of the screen. With this size, the open menu
# (x>=1508) does not cover the board (gap of 37 px).
C=147; X0=$(((1620-9*C)/2)); Y0=$(((2160-9*C)/2))

# Seven difficulty modes (0-6). LEVEL is the mode of the current puzzle. The New game panel opens at the last
# selected mode. At the first start, this is mode 2. The generator sets the techniques that each mode needs.
STOPS=7
LEVEL=2

# Difficulty meter: "Easy" to "Hard" is DIFF_W px wide. It is the same in the New game panel and below the board.
# Thus the New game panel is wider than 400 px (the width of the device menus).
DIFF_W=440
NG_PL=$((1507-DIFF_W-65))   # the left edge of the New game panel (inner margins of 33 px and 32 px)

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
