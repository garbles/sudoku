# MVG functions for the manual pages. The caller sets M (the MVG file). Cells use units of 140 px, with the scale K.
FONT="$(cd "$(dirname "$0")" && pwd)/fonts/rmsans-400.ttf"        # reMarkable Sans Regular, the UI font of the tablet
HEAD_FONT="$(cd "$(dirname "$0")" && pwd)/fonts/rmsans-500.ttf"   # Medium, for titles and headings
K=1.75; CW=245   # scale, and the cell width after the scale

# the center of micro-cell k (1-9) in a cell of 140 px
mc() { echo "$(( ($1-1)%3 * 140/3 + 23 )) $(( ($1-1)/3 * 140/3 + 23 ))"; }

open_cell() { printf 'push graphic-context\ntranslate %s,%s\nscale %s,%s\n' $1 $2 $K $K >> $M; }
close_cell() { printf 'stroke black stroke-width 3 fill none\nrectangle 0,0 140,140\npop graphic-context\n' >> $M; }

# cell X Y "digits" TAP DIGITCOLOR : a cell with micro-cells and pencil marks, and an optional tap ring
cell() {
  open_cell $1 $2
  echo "fill none stroke gray80 stroke-width 2 line 47,0 47,140 line 93,0 93,140 line 0,47 140,47 line 0,93 140,93" >> $M
  for d in $3; do c=$(mc $d)
    echo "stroke none fill ${5:-gray15} font '$FONT' font-size 30 text-anchor middle text ${c% *},$((${c#* }+11)) '$d'" >> $M
  done
  if [ -n "$4" ]; then c=$(mc $4)
    echo "fill none stroke black stroke-width 3 circle ${c% *},${c#* } $((${c% *}+20)),${c#* }" >> $M
  fi
  close_cell
}

# filled X Y DIGIT [BG] [COLOR] : a cell with a typed number (no micro-cells)
filled() {
  open_cell $1 $2
  [ -n "$4" ] && echo "stroke none fill $4 rectangle 0,0 140,140" >> $M
  echo "stroke none fill ${5:-black} font '$FONT' font-size 100 text-anchor middle text 70,106 '$3'" >> $M
  close_cell
}

# ink X Y "svg path" : a pen stroke on a cell. Use it after the cell function.
ink() {
  open_cell $1 $2
  echo "fill none stroke black stroke-width 5 stroke-linecap round stroke-linejoin round path '$3'" >> $M
  echo "pop graphic-context" >> $M
}

# rub X Y : an eraser stroke on a cell. Use it after the cell function.
rub() {
  open_cell $1 $2
  echo "fill none stroke gray50 stroke-opacity 0.45 stroke-width 22 stroke-linecap round stroke-linejoin round polyline 22,30 112,24 26,62 116,58 24,96 114,92 30,120" >> $M
  echo "pop graphic-context" >> $M
}

# Board colors. They are the same as in screen-in-progress.png.
GIVEN_BG=gray88; ENTRY_FG=gray35; CLASH_BG='#F6C4C4'; UNIT_BG='#DCE9F7'

# minib X Y "r,c,bg ..." "r,c,digit,color ..." : a small 9x9 board (cells of 34 px) with colored cells and digits
MC=34
minib() {
  for s in $3; do IFS=, read r c bg <<X
$s
X
    echo "stroke none fill $bg rectangle $(($1+c*MC)),$(($2+r*MC)) $(($1+c*MC+MC)),$(($2+r*MC+MC))" >> $M; done
  for s in $4; do IFS=, read r c d col <<X
$s
X
    echo "stroke none fill $col font '$FONT' font-size 24 text-anchor middle text $(($1+c*MC+17)),$(($2+r*MC+26)) '$d'" >> $M; done
  i=0; while [ $i -le 9 ]; do
    if [ $((i%3)) -eq 0 ]; then w=4; else w=1.5; fi
    echo "fill none stroke black stroke-width $w stroke-linecap square line $(($1+i*MC)),$2 $(($1+i*MC)),$(($2+9*MC)) line $1,$(($2+i*MC)) $(($1+9*MC)),$(($2+i*MC))" >> $M
    i=$((i+1)); done
}

# pagenum N TOTAL : the page number with the previous and next arrows (gray at the first and last page),
# and the "< Back" button that shows the game again (top left, at the same position as on the device)
pagenum() {
  text 702 1835 28 gray50 middle "$1 / $2"
  prev=black; next=black; [ $1 -eq 1 ] && prev=gray80; [ $1 -eq $2 ] && next=gray80
  echo "fill none stroke-width 3 stroke-linecap round stroke-linejoin round stroke $prev polyline 628,1815 618,1825 628,1835 stroke $next polyline 776,1815 786,1825 776,1835" >> $M
  echo "fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline 48,37 38,48 48,59" >> $M
  echo "stroke none fill black font '$HEAD_FONT' font-size 22 text-anchor start text 66,56 'Back'" >> $M
}

# arrow X1 X2 Y
arrow() {
  echo "fill black stroke black stroke-width 6 line $1,$3 $(($2-16)),$3 stroke none polygon $(($2-22)),$(($3-16)) $2,$3 $(($2-22)),$(($3+16))" >> $M
}

# text X Y SIZE COLOR ANCHOR "string"
text() { f=$FONT; [ $3 -ge 50 ] && f=$HEAD_FONT; echo "stroke none fill $4 font '$f' font-size $3 text-anchor $5 text $1,$2 \"$6\"" >> $M; }

# shot SRC GEOMETRY : cut an area of a UI image (device pixels) into a temporary file, and print its path
shot() { f=$(mktemp).png; magick "$1" -crop "$2" +repage "$f"; echo "$f"; }

# picture X Y FILE : put an image at its real size on the device (layout units are device px / SCALE)
picture() {
  w=$(magick identify -format %w "$3"); h=$(magick identify -format %h "$3")
  echo "image over $1,$2 $(awk -v w=$w -v s=$SCALE 'BEGIN{printf "%.1f", w/s}'),$(awk -v h=$h -v s=$SCALE 'BEGIN{printf "%.1f", h/s}') '$3'" >> $M
}

# render OUTFILE
# The layout uses units of 1404x1872. The scale changes it to the 1620x2160 screen of the Paper Pro.
SCALE=1.153846
wrap() { { printf "push graphic-context\nscale $SCALE,$SCALE\n"; cat $M; printf "pop graphic-context\n"; } > $M.s; mv $M.s $M; }
render() { wrap; magick -size 1620x2160 xc:white -draw @$M -units PixelsPerInch -density 229 "$1"; rm -f $M; }
