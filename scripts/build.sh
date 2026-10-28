#!/bin/sh
# Makes all the images in ../images from these scripts. You can run it from all folders: sh scripts/build.sh
# It also makes the AppLoad icon ../appload/sudoku/icon.png from ../icon.svg.
# It needs ImageMagick 7 (magick), rsvg-convert, python3 and bash. For the fonts, refer to fonts/README.md.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HERE/../images"
cd "$HERE"

magick -size 1620x2160 xc:white -units PixelsPerInch -density 229 "$OUT/template.png"
sh render.sh "$OUT"                          # screen.png, box.png, cell.png
for mode in progress highlight unrecognized completed; do
  name=screen-$mode.png; [ $mode = progress ] && name=screen-in-progress.png
  M=$(mktemp); python3 -I board-state.py $mode > $M
  magick "$OUT/screen.png" -draw @$M -units PixelsPerInch -density 229 "$OUT/$name"; rm -f $M
done
sh toolbar.sh "$OUT"                         # screens with the menu open, and the panels
python3 -I analytics.py "$OUT"               # analytics.png, analytics-games.png
for page in how-to-play candidates pen pen-rules helpers mistakes menu new-game finished; do
  sh manual-$page.sh "$OUT"
  sh manual-$page-landscape.sh "$OUT"          # the same page for the landscape orientation
done
# The terminal output of the installer (install.sh) in the simulation of tests/install/run.sh.
for scenario in first connect restart again; do
  T=$(mktemp); bash ../tests/install/run.sh $scenario > $T || true
  python3 -I terminal.py $T "$OUT/install-$scenario.png"; rm -f $T
done
# The AppLoad icon. AppLoad shows icon.png at 150x150 and does not read SVG files.
I=$(mktemp); rsvg-convert -w 150 -h 150 ../icon.svg -o $I
magick $I -colorspace gray -define png:exclude-chunks=date,time "$HERE/../appload/sudoku/icon.png"; rm -f $I
echo "built $(ls "$OUT"/*.png | wc -l | tr -d ' ') images in $OUT"
