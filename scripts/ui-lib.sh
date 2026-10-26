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
# Thus the New game panel is wider than 400 px (the width of the device menus). The color panel is 400 px wide.
DIFF_W=440
NG_PL=$((1507-DIFF_W-65))   # the left edge of the New game panel (inner margins of 33 px and 32 px)

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
