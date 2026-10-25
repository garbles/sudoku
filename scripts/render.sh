#!/bin/sh
# Makes screen.png, box.png and cell.png in $1. All use the same 1620x2160 coordinates (reMarkable Paper Pro).
# The board dimensions come from ui-lib.sh. Each cell has a faint 3x3 grid (one position for each pencil mark).
set -e
. "$(dirname "$0")/ui-lib.sh"
OUT=$1
FAINT=gray80

# lines for cells [c0,c1) x [r0,r1): prints the draw text for "faint", "thin" or "thick" lines
grid() { # c0 c1 r0 r1 kind
  awk -v X0=$X0 -v Y0=$Y0 -v C=$C -v c0=$1 -v c1=$2 -v r0=$3 -v r1=$4 -v kind=$5 'BEGIN{
    xa=X0+c0*C; xb=X0+c1*C; ya=Y0+r0*C; yb=Y0+r1*C; s=""
    if (kind=="faint") {
      for (c=c0;c<c1;c++) for (k=1;k<=2;k++) { x=X0+c*C+int(k*C/3+0.5); s=s sprintf("line %d,%d %d,%d ",x,ya,x,yb) }
      for (r=r0;r<r1;r++) for (k=1;k<=2;k++) { y=Y0+r*C+int(k*C/3+0.5); s=s sprintf("line %d,%d %d,%d ",xa,y,xb,y) }
    } else {
      for (i=c0;i<=c1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") s=s sprintf("line %d,%d %d,%d ",X0+i*C,ya,X0+i*C,yb) }
      for (i=r0;i<=r1;i++) { thick=(i%3==0); if ((kind=="thick")==thick || kind=="allthin") s=s sprintf("line %d,%d %d,%d ",xa,Y0+i*C,xb,Y0+i*C) }
    }
    print s }'
}

render() { # out c0 c1 r0 r1 thinkind extra_args... ; crop given via CROP env
  out=$1; c0=$2; c1=$3; r0=$4; r1=$5; tk=$6; shift 6
  thick=$( [ "$tk" = allthin ] && echo "" || grid $c0 $c1 $r0 $r1 thick )
  set -- -size 1620x2160 xc:white -fill none \
    -stroke $FAINT -strokewidth 2 -draw "$(grid $c0 $c1 $r0 $r1 faint)" \
    -stroke black -strokewidth 3 -draw "$(grid $c0 $c1 $r0 $r1 $tk)" "$@"
  [ -n "$thick" ] && set -- "$@" -strokewidth 12 -draw "stroke-linecap square $thick"
  [ -n "$CROP" ] && set -- "$@" -crop $CROP +repage
  magick "$@" -units PixelsPerInch -density 229 "$OUT/$out"
}

# screen (menu closed): the board, the menu button at the top right (closed icon, from the device) and the
# difficulty meter below the board. The close button shows only when the menu is open, as on the device (refer to toolbar.sh).
B=$((C*9)); DY=$((Y0+B+50)); DXL=$((X0+B/2-DIFF_W/2)); DXR=$((DXL+DIFF_W))   # the player cannot change it: it shows the difficulty of the current puzzle
CROP= render screen.png 0 9 0 9 thin
magick "$OUT/screen.png" -draw "$(labeled_slider $DXL $DXR $DY $LEVEL)" \
  "$UI_DIR/icons/ic-toggle-closed.png" -geometry +$TB_X+0 -composite \
  -units PixelsPerInch -density 229 "$OUT/screen.png"
# box: the top-left 3x3 box, with 20 px of space around it
CROP=$((3*C+40))x$((3*C+40))+$((X0-20))+$((Y0-20)) render box.png 0 3 0 3 thin
# cell: the center cell of the top-left box, with thin lines on all sides and 20 px of space around it
CROP=$((C+40))x$((C+40))+$((X0+C-20))+$((Y0+C-20)) render cell.png 1 2 1 2 allthin
