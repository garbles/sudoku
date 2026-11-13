#!/bin/sh
# Makes the files of an AppLoad folder with the QML frontend (REQUIREMENTS.md section 14.4, LCH-6),
# without the program `sudoku`:
#
#   appload/make-folder.sh OUT            the folder of Sudoku
#   appload/make-folder.sh OUT corpus     the folder of the corpus app `Sudoku (corpus)`
#
# OUT gets manifest.json, resources.rcc, backend/entry (mode 755), external.manifest.json and
# icon.png. The build adds the program. The Dockerfile, Dockerfile.dev and Dockerfile.corpus run
# this file in a stage that has the Qt tool `rcc` (the Alpine package qt6-qtbase-dev).
#
# resources.rcc is the Qt resource file with the QML of the frontend: binary format, no compression.
# The corpus app has the same frontend with its own name, app ID and framebuffer ID (LCH-5,
# `launch::CORPUS` in src/launch.rs): no two installed apps have the same IDs.
set -eu

src="$(cd "$(dirname "$0")" && pwd)/sudoku"
out="$1"
variant="${2:-sudoku}"
rcc="${RCC:-/usr/lib/qt6/libexec/rcc}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$src/ui" "$src/application.qrc" "$src/manifest.json" "$src/external.manifest.json" "$work/"

# Replaces the text $2 with the text $3 in the file $1. The text must be in the file exactly one time.
replace() {
  [ "$(grep -cF "$2" "$1")" = 1 ] || { echo "make-folder.sh: $1 does not have $2 one time" >&2; exit 1; }
  sed -i "s|$2|$3|" "$1"
  grep -qF "$3" "$1"
}

case "$variant" in
  sudoku) ;;
  corpus)
    replace "$work/manifest.json" '"name": "Sudoku"' '"name": "Sudoku (corpus)"'
    replace "$work/manifest.json" '"id": "sudoku"' '"id": "sudoku-corpus"'
    replace "$work/external.manifest.json" '"name": "Sudoku"' '"name": "Sudoku (corpus)"'
    replace "$work/ui/main.qml" 'appId: "sudoku"' 'appId: "sudoku-corpus"'
    replace "$work/ui/main.qml" 'framebufferId: 0x5355444F' 'framebufferId: 0x53554443'
    ;;
  *)
    echo "make-folder.sh: unknown app $variant" >&2
    exit 1
    ;;
esac

mkdir -p "$out/backend"
# A fixed time in the resource file, so that the same QML gives the same bytes.
QT_RCC_SOURCE_DATE_OVERRIDE=1 "$rcc" --binary --no-compress -o "$out/resources.rcc" "$work/application.qrc"
cp "$work/manifest.json" "$work/external.manifest.json" "$src/icon.png" "$out/"
cp "$src/backend/entry" "$out/backend/entry"
chmod 644 "$out/manifest.json" "$out/external.manifest.json" "$out/icon.png" "$out/resources.rcc"
chmod 755 "$out/backend/entry"
