#!/bin/sh
# Copies the screen of the reMarkable Paper Pro (1620x2160) through USB SSH into $1 (png). It does not change the device.
# It reads the BGRA framebuffer of xochitl (stride 1632 px) from /proc/<pid>/mem. The offsets come from an examination of the OS with Qt 6.10.
set -e
OUT=$1; T=$(mktemp)
ssh -o BatchMode=yes root@10.11.99.1 '
  P=$(pidof xochitl)
  END=$(grep card0 /proc/$P/maps | tail -n 1 | cut -d- -f2 | cut -d" " -f1)
  dd if=/proc/$P/mem bs=4096 skip=$((0x$END/4096)) count=5165 2>/dev/null' > $T
tail -c +$((3526640+540*6528+392*4+1)) $T | head -c $((6528*2160)) > $T.raw
magick -size 1632x2160 -depth 8 bgra:$T.raw -alpha off -crop 1620x2160+0+0 +repage "$OUT"
rm -f $T $T.raw
