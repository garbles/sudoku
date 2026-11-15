#!/bin/sh
# Makes the files of an AppLoad folder with the QML frontend (REQUIREMENTS.md section 14.4, LCH-6),
# without the program `sudoku`:
#
#   appload/make-folder.sh OUT
#
# OUT gets manifest.json, resources.rcc, backend/entry (mode 755), external.manifest.json and
# icon.png. The build adds the program. The Dockerfile and Dockerfile.dev run this file in a stage
# that has the Qt tool `rcc` (the Alpine package qt6-qtbase-dev).
#
# resources.rcc is the Qt resource file with the QML of the frontend: binary format, no compression.
set -eu

src="$(cd "$(dirname "$0")" && pwd)/sudoku"
out="$1"
rcc="${RCC:-/usr/lib/qt6/libexec/rcc}"

mkdir -p "$out/backend"
# A fixed time in the resource file, so that the same QML gives the same bytes.
QT_RCC_SOURCE_DATE_OVERRIDE=1 "$rcc" --binary --no-compress -o "$out/resources.rcc" "$src/application.qrc"
cp "$src/manifest.json" "$src/external.manifest.json" "$src/icon.png" "$out/"
cp "$src/backend/entry" "$out/backend/entry"
chmod 644 "$out/manifest.json" "$out/external.manifest.json" "$out/icon.png" "$out/resources.rcc"
chmod 755 "$out/backend/entry"
