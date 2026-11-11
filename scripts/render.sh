#!/bin/sh
# Makes screen.png, box.png and cell.png in $1. All use the same screen coordinates (the device of ui-lib.sh).
# The board dimensions come from ui-lib.sh. Each cell has a 3x3 grid of micro-cells (one position for each pencil mark):
# two vertical and two horizontal lines of 2 px, as a pattern of gray15 and white pixels (REQUIREMENTS.md §4.1,
# ui-lib.sh bayer_points). FAINT is the gray of the micro-cell lines (the setting of REQUIREMENTS.md §10; default
# gray90, as LineGray::default). Its level sets the darkness of the pattern. The pattern starts at the first pixel of
# each line, thus all cells have the same pixels: one cell makes a tile for the board. The board lines (black, 3 px
# and 13 px) have no anti-aliasing (REQUIREMENTS.md §15): the pixels whose centers are inside the stroke are black.
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

# lines for cells [c0,c1) x [r0,r1): prints the draw text for "thin", "thick" or "allthin" lines
grid() { # c0 c1 r0 r1 kind
  awk -v X0=$X0 -v Y0=$Y0 -v C=$C -v c0=$1 -v c1=$2 -v r0=$3 -v r1=$4 -v kind=$5 'BEGIN{
    xa=X0+c0*C; xb=X0+c1*C; ya=Y0+r0*C; yb=Y0+r1*C; s=""
    for (i=c0;i<=c1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") s=s sprintf("line %d,%d %d,%d ",X0+i*C,ya,X0+i*C,yb) }
    for (i=r0;i<=r1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") s=s sprintf("line %d,%d %d,%d ",xa,Y0+i*C,xb,Y0+i*C) }
    print s }'
}

render() { # out c0 c1 r0 r1 thinkind extra_args... ; crop given via CROP env
  out=$1; c0=$2; c1=$3; r0=$4; r1=$5; tk=$6; shift 6
  thick=$( [ "$tk" = allthin ] && echo "" || grid $c0 $c1 $r0 $r1 thick )
  # the micro-cell lines of the cells [c0,c1) x [r0,r1): the cell tile, repeated, then the board lines on top
  set -- -size ${PHYS_W}x${PHYS_H} xc:white \
    \( -size $(((c1-c0)*C))x$(((r1-r0)*C)) tile:"$TILE/cell.png" \) -geometry +$((X0+c0*C))+$((Y0+r0*C)) -composite -fill none \
    -stroke black -strokewidth 3 -draw "stroke-antialias 0 $(grid $c0 $c1 $r0 $r1 $tk)" "$@"
  [ -n "$thick" ] && set -- "$@" -strokewidth 12 -draw "stroke-antialias 0 stroke-linecap square $thick"
  [ -n "$CROP" ] && set -- "$@" -crop $CROP +repage
  magick "$@" -units PixelsPerInch -density $DPI "$OUT/$out"
}

# screen (menu closed): the board, the menu button at the top right (closed icon) and the
# difficulty meter below the board. The close button shows only when the menu is open, as on the device (refer to toolbar.sh).
B=$((C*9)); DY=$((Y0+B+50)); DXL=$((X0+B/2-DIFF_W/2)); DXR=$((DXL+DIFF_W))   # the player cannot change it: it shows the difficulty of the current puzzle
CROP= render screen.png 0 9 0 9 thin
magick "$OUT/screen.png" -draw "$(labeled_slider $DXL $DXR $DY $LEVEL)" -draw "$(menu_icon closed)" \
  -units PixelsPerInch -density $DPI "$OUT/screen.png"
# box: the top-left 3x3 box, with 20 px of space around it
CROP=$((3*C+40))x$((3*C+40))+$((X0-20))+$((Y0-20)) render box.png 0 3 0 3 thin
# cell: the center cell of the top-left box, with thin lines on all sides and 20 px of space around it
CROP=$((C+40))x$((C+40))+$((X0+C-20))+$((Y0+C-20)) render cell.png 1 2 1 2 allthin
rm -rf "$TILE"
