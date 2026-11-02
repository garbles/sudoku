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
# Scenarios:
#   first     A new tablet in developer mode: all steps operate.
#   connect   The tablet is not connected, then developer mode is off. All other steps are done.
#   again     All steps are done.
#   restart   After a restart of the tablet (xovi is off) with an earlier version of Sudoku.
#   twice     The scenario "first", then the installer again. The output is the output of the second run.
#   password  "first" with an incorrect password three times.
#   hostkey   The tablet has a new host key after a factory reset.
#   offline   The tablet has no internet at the first check. Vellum is not installed.
#   download  The computer has no internet at the first download.
#   stop      The tablet is not connected, and the user types q.
#   other     The tablet is a Paper Pro Move, and the user does not continue.
#   patch     All steps are done, but the ink patch is old: the installer copies it and starts xovi again.
#   stamp     All steps are done, but the stamp file of the ink patch is missing (an installer before the
#             stamp file): the installer writes the stamp file and does not start xovi again.
#   badpatch  The ink patch on GitHub does not agree with its checksum: the installer stops at step 2.
#   preview   All steps are done. SUDOKU_VERSION selects the preview version pre-1a2b3c4.
#   missing   SUDOKU_VERSION selects a version that GitHub does not have.
#   select    All steps are done. The user selects "A different version", then types an incorrect
#             number, a number that is not in the list and then pre-1a2b3c4 (number 3).
#   typed     As "select", but GitHub does not give the list. The user types pre-1a2b3c4.
#   wifi      All steps are done. REMARKABLE_HOST gives a Wi-Fi address. The first connection fails,
#             and the user selects "Try again".
#   wifi-usb  As "wifi", but Wi-Fi does not operate. The user selects "Use the USB cable".
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SCENARIO="${1:?usage: run.sh SCENARIO}"
SIM="$(mktemp -d "${TMPDIR:-/tmp}/sudoku-install-sim.XXXXXX")"
trap 'rm -rf "$SIM"' EXIT
export SIM
S="$SIM/state"
mkdir -p "$SIM/home" "$S" "$SIM/web"
if command -v sha256sum >/dev/null; then SUM=(sha256sum); else SUM=(shasum -a 256); fi

VERSION=v0.1.2
PASSWORD=Xk4pQ9wE2t

# The release archive of a fake app. It writes the checksum of the executable to stdout.
archive() { # version, archive file
  local d="$SIM/pkg-$1"
  mkdir -p "$d/sudoku"
  printf 'fake sudoku %s\n' "$1" >"$d/sudoku/sudoku"
  printf '{"name": "Sudoku"}\n' >"$d/sudoku/external.manifest.json"
  tar -C "$d" -czf "$2" sudoku
  "${SUM[@]}" "$d/sudoku/sudoku" | cut -d ' ' -f 1
}
APP="$(archive "$VERSION" "$SIM/web/sudoku-$VERSION.tar.gz")"
# The ink patch is a separate file of each release.
printf '; fake ink patch\n' >"$SIM/web/sudoku-penregion.qmd"
PATCH="$("${SUM[@]}" "$SIM/web/sudoku-penregion.qmd" | cut -d ' ' -f 1)"
OLD_APP="$(archive v0.1.1 "$SIM/old.tar.gz")"
WANT=""  # SUDOKU_VERSION of the user
RHOST="" # REMARKABLE_HOST of the user
SHA="$("${SUM[@]}" "$SIM/web/sudoku-$VERSION.tar.gz" | cut -d ' ' -f 1)"
# The same replacement as release.yml.
sed -e "s/@SUDOKU_VERSION@/$VERSION/" -e "s/@SUDOKU_SHA256@/$SHA/" -e "s/@SUDOKU_PATCH_SHA256@/$PATCH/" "$ROOT/install.sh" >"$SIM/install.sh"

printf '#!/bin/sh\n# Fake Vellum bootstrap\n' >"$SIM/web/bootstrap.sh"
printf '%s\n' "$PASSWORD" >"$S/password"
printf 'reMarkable Ferrari\n' >"$S/model"
printf '3.29.0.153\n' >"$S/os"

# The state after a complete installation.
installed() {
  mkdir -p "$SIM/home/.ssh"
  PATH="$HERE/fake:$PATH" ssh-keygen -q -t ed25519 -N "" -C sudoku-installer -f "$SIM/home/.ssh/id_ed25519_remarkable"
  cat "$SIM/home/.ssh/id_ed25519_remarkable.pub" >>"$S/authorized"
  touch "$S/vellum" "$S/appload" "$S/hashtab" "$S/xovi"
  printf '%s\n' "$APP" >"$S/app"
  printf '%s\n' "$PATCH" >"$S/patch"
  printf '%s\n' "$PATCH" >"$S/stamp"
}
# The release of the preview version $PRE, with its version.json.
PRE=pre-1a2b3c4
preview() {
  mkdir -p "$SIM/web/$PRE"
  archive "$PRE" "$SIM/web/sudoku-$PRE.tar.gz" >/dev/null
  printf '; fake ink patch\n' >"$SIM/web/$PRE/sudoku-penregion.qmd"
  printf '{"version": "%s", "url": "x", "sha256": "%s", "patch_url": "x", "patch_sha256": "%s"}\n' "$PRE" \
    "$("${SUM[@]}" "$SIM/web/sudoku-$PRE.tar.gz" | cut -d ' ' -f 1)" "$PATCH" >"$SIM/web/$PRE/version.json"
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
  restart) installed; rm "$S/xovi"; printf '%s\n' "$OLD_APP" >"$S/app"; input "" "" ;;
  twice) input "" "$PASSWORD" ""; run >/dev/null; input "" ;;
  password) input "" wrong1 wrong2 wrong3 "" "$PASSWORD" "" ;;
  hostkey) installed; touch "$S/hostkey-changed"; input "" y ;;
  offline) installed; rm "$S/vellum" "$S/appload"; echo 1 >"$S/offline"; input "" "" ;;
  download) installed; echo 1 >"$S/no-network"; input "" "" ;;
  stop) echo 9 >"$S/unreachable"; input "" q ;;
  other) installed; printf 'reMarkable Chiappa\n' >"$S/model"; input "" "" ;;
  patch) installed; printf 'old\n' >"$S/patch"; printf 'old\n' >"$S/stamp"; input "" "" ;;
  stamp) installed; rm "$S/stamp"; input "" ;;
  badpatch) installed; printf 'changed\n' >"$SIM/web/sudoku-penregion.qmd"; input "" ;;
  preview) installed; WANT=$PRE; preview; input ;;
  missing) installed; WANT=v9.9.9; input ;;
  select) installed; preview; releases; input 2 x 5 3 ;;
  typed) installed; preview; input 2 "$PRE" ;;
  wifi) installed; RHOST=192.168.1.20; echo 1 >"$S/wifi-unreachable"; input "" "" ;;
  wifi-usb) installed; RHOST=192.168.1.20; touch "$S/wifi-down"; input "" 2 ;;
  *) echo "run.sh: unknown scenario: $SCENARIO" >&2; exit 2 ;;
esac
run
