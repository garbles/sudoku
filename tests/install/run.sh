#!/usr/bin/env bash
# Runs install.sh in a simulation and writes the terminal output (with ANSI colors) to stdout.
#
#   bash tests/install/run.sh SCENARIO
#
# The fake programs in fake/ (ssh, ssh-keygen, curl) take the place of the real programs. The fake
# ssh simulates the tablet (refer to fake/ssh). A file gives the answers of the user. The exit code
# is the exit code of install.sh. tty.py runs a scenario in a terminal and pushes keys. tests/install.rs runs all scenarios, and scripts/build.sh makes the
# images install-*.png from some of them.
#
# The releases of the simulation (R§14.6, owner decisions 2026-11-12):
#   v0.1.2       The latest release, with the frontend: its folder sudoku/ has sudoku, manifest.json,
#                resources.rcc, backend/entry, external.manifest.json and icon.png. No ink patch.
#   pre-1a2b3c4  A preview release with the frontend. No ink patch.
#   v0.1.1       A release before the frontend: sudoku, external.manifest.json and icon.png, and the
#                ink patch as a separate file (version.json has patch_url and patch_sha256).
#
# Scenarios:
#   first     A new tablet in developer mode: all steps operate. The app folder is new, thus xovi
#             starts again after the copy.
#   connect   The tablet is not connected, then developer mode is off. All other steps are done.
#   again     All steps are done.
#   restart   After a restart of the tablet (xovi is off) with an earlier version of Sudoku with the
#             frontend. The power-button starter stays on.
#   twice     The scenario "first", then the installer again. The output is the output of the second run.
#   password  "first" with an incorrect password three times.
#   hostkey   The tablet has a new host key after a factory reset.
#   offline   The tablet has no internet at the first check. Vellum is not installed.
#   download  The computer has no internet at the first download.
#   stop      The tablet is not connected, and the user types q.
#   other     The tablet is a reMarkable 2, and the user does not continue.
#   move      The scenario "again" on a Paper Pro Move: the installer accepts the tablet with no question.
#   upgrade   The tablet has v0.1.1 (before the frontend) with its ink patch and stamp file. The
#             installer deletes the patch and the stamp file, replaces the app and starts xovi again
#             after the copy (the type of the manifest changed).
#   upgrade-again
#             The scenario "upgrade", then the installer again. The output is the output of the second run.
#   upgrade-appload
#             As "upgrade", but the AppLoad on the tablet is older than 0.6.0. The installer updates
#             AppLoad, and xovi starts one time, after the copy.
#   leftover  All steps are done with the frontend, but the old ink patch and its stamp file stay on
#             the tablet. The installer deletes them and does not start xovi again.
#   patch-delete-fail
#             As "leftover", but the deletion does nothing. The installer stops at step 8.
#   appload-old
#             All steps are done, but the AppLoad on the tablet is older than 0.6.0 (it has
#             DisplayMethodArea, but not fbRotation). The installer updates AppLoad and starts xovi
#             again at the end.
#   appload-fail
#             As "appload-old", but Vellum cannot update AppLoad. The installer stops at step 6.
#   appload-stays-old
#             As "appload-old", but after the update AppLoad is still older than 0.6.0. The installer
#             stops at step 6.
#   nofolder  All steps are done, but the AppLoad folder of Sudoku is not on the tablet. The installer
#             copies it and starts xovi again.
#   files     All steps are done, but resources.rcc on the tablet is not the file of the release (the
#             program sudoku is correct). The installer replaces the app and does not start xovi.
#   old-release
#             The tablet has v0.1.2. SUDOKU_VERSION selects v0.1.1, a release before the frontend: the
#             installer installs its ink patch as before, and starts xovi again for the patch and after
#             the copy.
#   old-release-again
#             The scenario "old-release", then the installer again. The output is the output of the
#             second run.
#   patch     The tablet has v0.1.1 with an old ink patch. SUDOKU_VERSION selects v0.1.1: the installer
#             copies its ink patch and starts xovi again.
#   stamp     As "patch", but the ink patch is correct and its stamp file is missing (an installer
#             before the stamp file): the installer writes the stamp file and does not start xovi again.
#   badpatch  SUDOKU_VERSION selects v0.1.1, and its ink patch on GitHub does not agree with its
#             checksum: the installer stops at step 2.
#   preview   All steps are done. SUDOKU_VERSION selects the preview version pre-1a2b3c4.
#   missing   SUDOKU_VERSION selects a version that GitHub does not have.
#   select    All steps are done. The user selects "A different version", then types an incorrect
#             number, a number that is not in the list and then pre-1a2b3c4 (number 3).
#   typed     As "select", but GitHub does not give the list. The user types pre-1a2b3c4.
#   wifi      All steps are done. REMARKABLE_HOST gives a Wi-Fi address. The first connection fails,
#             and the user selects "Try again".
#   wifi-usb  As "wifi", but Wi-Fi does not operate. The user selects "Use the USB cable".
#   tablet-update
#             The tablet software was updated: the Vellum state is stale, the hashtab file stays,
#             and the power-button unit is missing. Step 7 syncs Vellum and rebuilds the hashtab,
#             step 9 restarts xovi, and steps 10 and 11 are done.
#   tablet-update-sync-fail
#             As "tablet-update", but sync-vellum fails. The installer stops at step 7.
#   tablet-update-still-stale
#             As "tablet-update", but the Vellum state is still stale after sync-vellum. The
#             installer stops at step 7.
#   tripletap-repair
#             All steps are done, but the power-button service is off. Vellum installs the starter,
#             the service stays off, and the repair with the starter's enable.sh starts it.
#   tripletap-repair-fail
#             As "tripletap-repair", but the repair (with vellum reenable) does not start the
#             service. The installer stops before Sudoku.
#   tripletap-install-fail
#             All steps are done, but Vellum cannot install the power-button starter. The installer
#             stops before Sudoku.
#   tripletap-offline
#             All steps are done, but the tablet has no internet at the power-button step. The user
#             connects the tablet to Wi-Fi and continues.
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SCENARIO="${1:?usage: run.sh SCENARIO}"
SIM="$(mktemp -d "${TMPDIR:-/tmp}/sudoku-install-sim.XXXXXX")"
trap 'rm -rf "$SIM"' EXIT
export SIM
S="$SIM/state"
# /home/root of the simulated tablet (refer to fake/ssh).
TABLET="$SIM/tablet/home/root"
APPLOAD_SO="$TABLET/xovi/extensions.d/appload.so"
APPS="$TABLET/xovi/exthome/appload"
PATCHES="$TABLET/xovi/exthome/qt-resource-rebuilder"
mkdir -p "$SIM/home" "$S" "$SIM/web"
if command -v sha256sum >/dev/null; then SUM=(sha256sum); else SUM=(shasum -a 256); fi
sum() { "${SUM[@]}" "$1" | cut -d ' ' -f 1; }

VERSION=v0.1.2
OLD=v0.1.1
PASSWORD=Xk4pQ9wE2t

# The release archive of a fake app: frontend (a release with the frontend) or external (a release
# before the frontend). backend/entry has no executable mode in the archive: the installer sets it.
archive() { # version, archive file, kind
  local d="$SIM/pkg-$1"
  mkdir -p "$d/sudoku"
  printf 'fake sudoku %s\n' "$1" >"$d/sudoku/sudoku"
  printf '{"name": "Sudoku"}\n' >"$d/sudoku/external.manifest.json"
  printf 'fake icon\n' >"$d/sudoku/icon.png"
  if [ "$3" = frontend ]; then
    mkdir -p "$d/sudoku/backend"
    printf '{"id": "sudoku", "name": "Sudoku"}\n' >"$d/sudoku/manifest.json"
    printf 'fake qml %s\n' "$1" >"$d/sudoku/resources.rcc"
    printf '#!/bin/sh\nexec "$(dirname "$0")/../sudoku" "$@"\n' >"$d/sudoku/backend/entry"
  fi
  tar -C "$d" -czf "$2" sudoku
}
archive "$VERSION" "$SIM/web/sudoku-$VERSION.tar.gz" frontend
# An earlier build with the frontend (only on the tablet).
archive pre-0e1d2c3 "$SIM/earlier.tar.gz" frontend
# The release before the frontend, with its version.json and its ink patch.
archive "$OLD" "$SIM/web/sudoku-$OLD.tar.gz" external
mkdir -p "$SIM/web/$OLD"
printf '; fake ink patch\n' >"$SIM/web/$OLD/sudoku-penregion.qmd"
PATCH="$(sum "$SIM/web/$OLD/sudoku-penregion.qmd")"
printf '{"version": "%s", "url": "x", "sha256": "%s", "patch_url": "x", "patch_sha256": "%s"}\n' "$OLD" \
  "$(sum "$SIM/web/sudoku-$OLD.tar.gz")" "$PATCH" >"$SIM/web/$OLD/version.json"
WANT=""  # SUDOKU_VERSION of the user
RHOST="" # REMARKABLE_HOST of the user
SHA="$(sum "$SIM/web/sudoku-$VERSION.tar.gz")"
# The same replacement as release.yml.
sed -e "s/@SUDOKU_VERSION@/$VERSION/" -e "s/@SUDOKU_SHA256@/$SHA/" "$ROOT/install.sh" >"$SIM/install.sh"

printf '#!/bin/sh\n# Fake Vellum bootstrap\n' >"$SIM/web/bootstrap.sh"
printf '%s\n' "$PASSWORD" >"$S/password"
printf 'reMarkable Ferrari\n' >"$S/model"
printf '3.29.0.153\n' >"$S/os"

# The AppLoad folder of Sudoku on the tablet: the folder sudoku/ of the archive.
put_app() { # archive file
  rm -rf "$APPS/sudoku"
  mkdir -p "$APPS"
  tar -xzf "$1" -C "$APPS"
  chmod 755 "$APPS/sudoku/sudoku"
  [ ! -f "$APPS/sudoku/backend/entry" ] || chmod 755 "$APPS/sudoku/backend/entry"
}
# The ink patch of v0.1.1 and its stamp file, as the installer of v0.1.1 wrote them.
put_patch() { # checksum in the stamp file (default: the checksum of the patch)
  mkdir -p "$PATCHES"
  cp "$SIM/web/$OLD/sudoku-penregion.qmd" "$PATCHES/sudoku-penregion.qmd"
  printf '{"version": "%s", "sha256": "%s"}\n' "$OLD" "${1:-$PATCH}" >"$PATCHES/sudoku-penregion.json"
}
# AppLoad 0.5.1 to 0.5.3: it has DisplayMethodArea (ApploadUtils), but not fbRotation (A11).
old_appload() { printf 'appload 0.5.3 ApploadUtils\n' >"$APPLOAD_SO"; }

# The state after a complete installation of v0.1.2.
installed() {
  mkdir -p "$SIM/home/.ssh" "$(dirname "$APPLOAD_SO")"
  PATH="$HERE/fake:$PATH" ssh-keygen -q -t ed25519 -N "" -C sudoku-installer -f "$SIM/home/.ssh/id_ed25519_remarkable"
  cat "$SIM/home/.ssh/id_ed25519_remarkable.pub" >>"$S/authorized"
  touch "$S/vellum" "$S/hashtab" "$S/xovi" "$S/tripletap"
  printf 'appload 0.6.0 ApploadUtils fbRotation\n' >"$APPLOAD_SO"
  put_app "$SIM/web/sudoku-$VERSION.tar.gz"
}
# The state after a complete installation of v0.1.1 (before the frontend), with its ink patch.
installed_old() {
  installed
  put_app "$SIM/web/sudoku-$OLD.tar.gz"
  put_patch
}
# The release of the preview version $PRE (with the frontend), with its version.json.
PRE=pre-1a2b3c4
preview() {
  mkdir -p "$SIM/web/$PRE"
  archive "$PRE" "$SIM/web/sudoku-$PRE.tar.gz" frontend
  printf '{"version": "%s", "url": "x", "sha256": "%s"}\n' "$PRE" "$(sum "$SIM/web/sudoku-$PRE.tar.gz")" \
    >"$SIM/web/$PRE/version.json"
}
# The list of releases of the GitHub API (compact JSON, the newest first).
releases() {
  local t sep="" json="["
  for t in "pre-9f8e7d6 true 2026-11-02T09:12:40Z" "v0.1.2 false 2026-11-01T06:30:29Z" \
    "$PRE true 2026-11-01T05:35:31Z" "v0.1.1 false 2026-10-28T05:14:30Z"; do
    set -- $t
    json="$json$sep{\"tag_name\":\"$1\",\"name\":\"$1\",\"draft\":false,\"prerelease\":$2,\"published_at\":\"$3\",\"assets\":[{\"name\":\"version.json\"}],\"body\":\"Notes with \\\"tag_name\\\": \\\"x\\\"\"}"
    sep=","
  done
  printf '%s]\n' "$json" >"$SIM/web/releases"
}
input() { printf '%s\n' "$@" >"$SIM/input"; }
run() {
  # The prompt line of the terminal.
  [ -z "$WANT" ] || printf '\033[38;5;114m$\033[0m export SUDOKU_VERSION=%s\n' "$WANT"
  [ -z "$RHOST" ] || printf '\033[38;5;114m$\033[0m export REMARKABLE_HOST=%s\n' "$RHOST"
  printf '\033[38;5;114m$\033[0m curl -fsSL https://github.com/garbles/sudoku/releases/latest/download/install.sh | bash\n'
  # RUN_TTY=1 (tty.py): the answers come from the terminal, not from the file.
  local tty="$SIM/input"
  [ "${RUN_TTY:-}" = 1 ] && tty=/dev/tty
  env -i PATH="$HERE/fake:$PATH" HOME="$SIM/home" SIM="$SIM" CLICOLOR_FORCE=1 ${NO_COLOR:+NO_COLOR=1} ${WANT:+SUDOKU_VERSION=$WANT} ${RHOST:+REMARKABLE_HOST=$RHOST} \
    SUDOKU_INSTALL_TTY="$tty" bash "$SIM/install.sh" 2>&1
}

case "$SCENARIO" in
  first) input "" "$PASSWORD" "" ;;
  connect) installed; echo 1 >"$S/unreachable"; echo 1 >"$S/refused"; input "" "" "" ;;
  again) installed; input "" ;;
  restart) installed; rm "$S/xovi"; put_app "$SIM/earlier.tar.gz"; input "" "" ;;
  twice) input "" "$PASSWORD" ""; run >/dev/null; input "" ;;
  password) input "" wrong1 wrong2 wrong3 "" "$PASSWORD" "" ;;
  hostkey) installed; touch "$S/hostkey-changed"; input "" y ;;
  offline) installed; rm "$S/vellum" "$APPLOAD_SO"; echo 1 >"$S/offline"; input "" "" ;;
  download) installed; echo 1 >"$S/no-network"; input "" "" ;;
  stop) echo 9 >"$S/unreachable"; input "" q ;;
  other) installed; printf 'reMarkable 2.0\n' >"$S/model"; input "" "" ;;
  move) installed; printf 'reMarkable Chiappa\n' >"$S/model"; input "" ;;
  upgrade) installed_old; input "" "" ;;
  upgrade-again) installed_old; input "" ""; run >/dev/null; input "" ;;
  upgrade-appload) installed_old; old_appload; input "" "" ;;
  leftover) installed; put_patch; input "" ;;
  patch-delete-fail) installed; put_patch; touch "$S/patch-delete-fails"; input "" ;;
  appload-old) installed; old_appload; input "" "" ;;
  appload-fail) installed; old_appload; touch "$S/appload-update-fails"; input "" ;;
  appload-stays-old) installed; old_appload; touch "$S/appload-update-noop"; input "" ;;
  nofolder) installed; rm -rf "$APPS/sudoku"; input "" "" ;;
  files) installed; printf 'fake qml of a different build\n' >"$APPS/sudoku/resources.rcc"; input "" ;;
  old-release) installed; WANT=$OLD; input "" ;;
  old-release-again) installed; WANT=$OLD; input ""; run >/dev/null; input ;;
  patch)
    installed_old; WANT=$OLD
    printf '; an older ink patch\n' >"$SIM/older.qmd"
    put_patch "$(sum "$SIM/older.qmd")"; cp "$SIM/older.qmd" "$PATCHES/sudoku-penregion.qmd"
    input ""
    ;;
  stamp) installed_old; WANT=$OLD; rm "$PATCHES/sudoku-penregion.json"; input ;;
  badpatch) installed; WANT=$OLD; printf 'changed\n' >"$SIM/web/$OLD/sudoku-penregion.qmd"; input ;;
  preview) installed; WANT=$PRE; preview; input ;;
  missing) installed; WANT=v9.9.9; input ;;
  select) installed; preview; releases; input 2 x 5 3 ;;
  typed) installed; preview; input 2 "$PRE" ;;
  wifi) installed; RHOST=192.168.1.20; echo 1 >"$S/wifi-unreachable"; input "" "" ;;
  wifi-usb) installed; RHOST=192.168.1.20; touch "$S/wifi-down"; input "" 2 ;;
  tablet-update) installed; rm "$S/tripletap"; touch "$S/vellum-stale"; input "" "" ;;
  tablet-update-sync-fail) installed; rm "$S/tripletap"; touch "$S/vellum-stale" "$S/vellum-sync-fails"; input "" "" ;;
  tablet-update-still-stale) installed; rm "$S/tripletap"; touch "$S/vellum-stale" "$S/vellum-still-stale"; input "" "" ;;
  tripletap-repair) installed; rm "$S/tripletap"; touch "$S/tripletap-off" "$S/tripletap-enable-sh"; input "" ;;
  tripletap-repair-fail) installed; rm "$S/tripletap"; touch "$S/tripletap-off" "$S/tripletap-repair-fails"; input "" ;;
  tripletap-install-fail) installed; rm "$S/tripletap"; touch "$S/tripletap-install-fails"; input "" ;;
  tripletap-offline) installed; rm "$S/tripletap"; echo 1 >"$S/offline"; input "" "" ;;
  *) echo "run.sh: unknown scenario: $SCENARIO" >&2; exit 2 ;;
esac
run
