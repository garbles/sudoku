#!/bin/sh
# Makes all the images in ../images from these scripts. You can run it from all folders: sh scripts/build.sh
# It also makes the AppLoad icon ../appload/sudoku/icon.png from ../icon.svg.
# It needs ImageMagick 7 (magick), rsvg-convert, python3 and bash. For the fonts, refer to fonts/README.md.
#
#   [DEVICE=pro] sh scripts/build.sh [screens] [installer] [icon]
#
# The arguments select the parts. With no argument, the script makes all parts:
#   screens    the design images of the tablet model DEVICE (default: pro, devices/$DEVICE.conf) in
#              ../images/$DEVICE: all images of that folder except icons.png. Dockerfile.images.
#              The snapshot tool makes icons.png (also in Dockerfile.images).
#   installer  the terminal output of the installer (../images/install-*.png). It is the same for all
#              models. Dockerfile.installer.
#   icon       the AppLoad icon. Dockerfile.images.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
SCRIPTS_DIR="$HERE"; . ./device-lib.sh
OUT="$HERE/../images/$DEVICE"                # the design images of the device
INSTALL_OUT="$HERE/../images"                # the installer images (the same for all devices)
PARTS="${*:-screens installer icon}"
part() { case " $PARTS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
for p in $PARTS; do
  case $p in screens|installer|icon) ;; *) echo "unknown part: $p" >&2; exit 2 ;; esac
done

if part screens; then
mkdir -p "$OUT"
magick -size ${PHYS_W}x${PHYS_H} xc:white -units PixelsPerInch -density $DPI "$OUT/template.png"
sh render.sh "$OUT"                          # screen.png, box.png, cell.png
for mode in progress highlight unrecognized invalid-mark completed-group completed; do
  name=screen-$mode.png; [ $mode = progress ] && name=screen-in-progress.png
  M=$(mktemp); python3 -I board-state.py $mode > $M
  magick "$OUT/screen.png" -draw @$M -units PixelsPerInch -density $DPI "$OUT/$name"; rm -f $M
done
sh toolbar.sh "$OUT"                         # screens with the menu open, and the panels
python3 -I analytics.py "$OUT"               # analytics.png, analytics-games.png
for page in how-to-play candidates pen pen-rules helpers mistakes menu new-game finished; do
  sh manual-$page.sh "$OUT"
  sh manual-$page-landscape.sh "$OUT"          # the same page for the landscape orientation
done
fi

if part installer; then
# The terminal output of the installer (install.sh) in the simulation of tests/install/run.sh.
for scenario in first connect restart again; do
  T=$(mktemp); bash ../tests/install/run.sh $scenario > $T || true
  python3 -I terminal.py $T "$INSTALL_OUT/install-$scenario.png"; rm -f $T
done
# The version menus in a terminal (tests/install/tty.py pushes the keys): the list of versions is
# open, then the complete run after the selection of a preview version.
T=$(mktemp); python3 -I ../tests/install/tty.py select down enter down down wait > $T
python3 -I terminal.py $T "$INSTALL_OUT/install-menu.png"
python3 -I ../tests/install/tty.py select down enter down down enter > $T
python3 -I terminal.py $T "$INSTALL_OUT/install-select.png"; rm -f $T
fi

if part icon; then
# The AppLoad icon. AppLoad shows icon.png at 150x150 and does not read SVG files.
I=$(mktemp); rsvg-convert -w 150 -h 150 ../icon.svg -o $I
magick $I -colorspace gray -define png:exclude-chunks=date,time "$HERE/../appload/sudoku/icon.png"; rm -f $I
fi
if part screens; then echo "built $(ls "$OUT"/*.png | wc -l | tr -d ' ') images in $OUT"; fi
if part installer; then echo "built $(ls "$INSTALL_OUT"/install-*.png | wc -l | tr -d ' ') installer images in $INSTALL_OUT"; fi
