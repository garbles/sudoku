#!/usr/bin/env bash
# Runs install.sh in a simulation and writes the terminal output (with ANSI colors) to stdout.
#
#   bash tests/install/run.sh SCENARIO
#
# The fake programs in fake/ (ssh, ssh-keygen, curl) take the place of the real programs. The fake
# ssh simulates the tablet (refer to fake/ssh). A file gives the answers of the user. The exit code
# is the exit code of install.sh. tests/install.rs runs all scenarios, and scripts/build.sh makes the
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
OLD_APP="$(archive v0.1.1 "$SIM/old.tar.gz")"
SHA="$("${SUM[@]}" "$SIM/web/sudoku-$VERSION.tar.gz" | cut -d ' ' -f 1)"
# The same replacement as release.yml.
sed -e "s/@SUDOKU_VERSION@/$VERSION/" -e "s/@SUDOKU_SHA256@/$SHA/" "$ROOT/install.sh" >"$SIM/install.sh"

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
}
input() { printf '%s\n' "$@" >"$SIM/input"; }
run() {
  # The prompt line of the terminal.
  printf '\033[38;5;114m$\033[0m curl -fsSL https://github.com/garbles/sudoku/releases/latest/download/install.sh | bash\n'
  env -i PATH="$HERE/fake:$PATH" HOME="$SIM/home" SIM="$SIM" CLICOLOR_FORCE=1 ${NO_COLOR:+NO_COLOR=1} \
    SUDOKU_INSTALL_TTY="$SIM/input" bash "$SIM/install.sh" 2>&1
}

case "$SCENARIO" in
  first) input "$PASSWORD" "" ;;
  connect) installed; echo 1 >"$S/unreachable"; echo 1 >"$S/refused"; input "" "" ;;
  again) installed; input ;;
  restart) installed; rm "$S/xovi"; printf '%s\n' "$OLD_APP" >"$S/app"; input "" ;;
  twice) input "$PASSWORD" ""; run >/dev/null; input ;;
  password) input wrong1 wrong2 wrong3 "" "$PASSWORD" "" ;;
  hostkey) installed; touch "$S/hostkey-changed"; input y ;;
  offline) installed; rm "$S/vellum" "$S/appload"; echo 1 >"$S/offline"; input "" ;;
  download) installed; echo 1 >"$S/no-network"; input "" ;;
  stop) echo 9 >"$S/unreachable"; input q ;;
  other) installed; printf 'reMarkable Chiappa\n' >"$S/model"; input "" ;;
  *) echo "run.sh: unknown scenario: $SCENARIO" >&2; exit 2 ;;
esac
run
