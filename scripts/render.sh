#!/bin/sh
# Makes screen.png, box.png and cell.png in $1. All use the same screen coordinates (the device of ui-lib.sh).
# The board dimensions come from ui-lib.sh. Each cell has a 3x3 grid of micro-cells (one position for each pencil mark):
# two vertical and two horizontal lines of 2 px, as a pattern of gray15 and white pixels (REQUIREMENTS.md §4.1,
# ui-lib.sh bayer_points). FAINT is the gray of the micro-cell lines (the setting of REQUIREMENTS.md §10; default
# gray90, as LineGray::default). Its level sets the darkness of the pattern. The pattern starts at the first pixel of
# each line, thus all cells have the same pixels: one cell makes a tile for the board. The board lines (black, 3 px
# and 7 px, each on its own pixels between the cells) are filled rectangles with no anti-aliasing (REQUIREMENTS.md §15).
set -e
. "$(dirname "$0")/ui-lib.sh"
OUT=$1
FAINT=${FAINT:-gray90}

# cell_lines : the MVG of the micro-cell lines of one cell, with the cell origin at 0,0
cell_lines() {
  g=$(gray_level $FAINT); last=$((C-1))
  for o in $MICRO_OFF1 $MICRO_OFF2; do
    bayer_points $g $o 0 $o 0 $((o+1)) $last y    # the vertical line: origin (o, 0), the row index along y
    bayer_points $g 0 $o 0 $o $last $((o+1)) x    # the horizontal line: origin (0, o), the row index along x
  done
}
TILE=$(mktemp -d)
magick -size ${C}x${C} xc:none -draw "$(cell_lines)" "$TILE/cell.png"

# lines for cells [c0,c1) x [r0,r1): prints the draw text (filled rectangles) for the "thin" lines (the cell lines), the
# "thick" lines (the border and the box lines) or "allthin" (all lines with the width of a cell line). Each line has
# its own pixels between the cells (ui-lib.sh line_off, src/layout.rs line_offset).
grid() { # c0 c1 r0 r1 kind
  awk -v X0=$X0 -v Y0=$Y0 -v C=$C -v BL=$BOX_LINE -v CL=$CELL_LINE -v c0=$1 -v c1=$2 -v r0=$3 -v r1=$4 -v kind=$5 '
    function off(k) { return k*C + int((k+2)/3)*BL + (k-int((k+2)/3))*CL }
    function w(k) { return (k%3==0) ? BL : CL }
    BEGIN{
    xa=X0+off(c0); xb=X0+off(c1)+w(c1)-1; ya=Y0+off(r0); yb=Y0+off(r1)+w(r1)-1; s=""
    for (i=c0;i<=c1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") { lw=(kind=="allthin")?CL:w(i); s=s sprintf("rectangle %d,%d %d,%d ",X0+off(i),ya,X0+off(i)+lw-1,yb) } }
    for (i=r0;i<=r1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") { lw=(kind=="allthin")?CL:w(i); s=s sprintf("rectangle %d,%d %d,%d ",xa,Y0+off(i),xb,Y0+off(i)+lw-1) } }
    print s }'
}

render() { # out c0 c1 r0 r1 thinkind extra_args... ; crop given via CROP env
  out=$1; c0=$2; c1=$3; r0=$4; r1=$5; tk=$6; shift 6
  thick=$( [ "$tk" = allthin ] && echo "" || grid $c0 $c1 $r0 $r1 thick )
  # the micro-cell lines of the cells [c0,c1) x [r0,r1): the cell tile at each cell, then the board lines on top
  set -- -size ${PHYS_W}x${PHYS_H} xc:white
  r=$r0; while [ $r -lt $r1 ]; do
    c=$c0; while [ $c -lt $c1 ]; do
      set -- "$@" "$TILE/cell.png" -geometry +$((X0+$(cell_off $c)))+$((Y0+$(cell_off $r))) -composite
      c=$((c+1)); done
    r=$((r+1)); done
  set -- "$@" -stroke none -fill black -draw "$(grid $c0 $c1 $r0 $r1 $tk)"
  [ -n "$thick" ] && set -- "$@" -draw "$thick"
  [ -n "$CROP" ] && set -- "$@" -crop $CROP +repage
  magick "$@" -units PixelsPerInch -density $DPI "$OUT/$out"
}

# screen (menu closed): the board, the menu button at the top right (closed icon) and the
# difficulty meter below the board. The close button shows only when the menu is open, as on the device (refer to toolbar.sh).
B=$BOARD; DY=$((Y0+B+50)); DXL=$((X0+B/2-DIFF_W/2)); DXR=$((DXL+DIFF_W))   # the player cannot change it: it shows the difficulty of the current puzzle
CROP= render screen.png 0 9 0 9 thin
magick "$OUT/screen.png" -draw "$(labeled_slider $DXL $DXR $DY $LEVEL)" -draw "$(menu_icon closed)" \
  -units PixelsPerInch -density $DPI "$OUT/screen.png"
# box: the top-left 3x3 box with its lines, and 14 px of space around it
BOXW=$(($(line_off 3)+BOX_LINE+28)); CROP=${BOXW}x$BOXW+$((X0-14))+$((Y0-14)) render box.png 0 3 0 3 thin
# cell: the center cell of the top-left box, with its thin lines on all sides and 18 px of space around them
CELLW=$((C+2*CELL_LINE+36)); CROP=${CELLW}x$CELLW+$((X0+$(line_off 1)-18))+$((Y0+$(line_off 1)-18)) render cell.png 1 2 1 2 allthin
rm -rf "$TILE"
