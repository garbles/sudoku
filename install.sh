#!/usr/bin/env bash
# Installs Sudoku on a reMarkable Paper Pro. Each release has this file:
#
#   curl -fsSL https://github.com/garbles/sudoku/releases/latest/download/install.sh | bash
#
# The script runs on your computer (macOS or Linux). It connects to the tablet with SSH and does the
# steps below. It examines each step first and skips it if it is done. Thus you can run the script
# again at all times, for example after a restart of the tablet (xovi then stops) or for a new
# version of Sudoku.
#
#   1. Examine the programs on this computer (ssh, ssh-keygen, curl, tar).
#   2. Select the version (the latest version, or a version from a list of all releases). Download
#      the release archive and the ink patch of the release, and examine their SHA-256 checksums.
#   3. Find the tablet (USB cable, developer mode).
#   4. Make an SSH key and add it to the tablet, with a different SSH key of this computer or with
#      the root password (the user types it one time).
#   5. Install Vellum, the package manager.
#   6. Install AppLoad (with xovi) with Vellum.
#   7. Make the hashtable of xovi.
#   8. Install the ink patch (a QML patch that xovi applies when it starts) and its stamp file (the
#      version that installed the patch and the checksum of the patch).
#   9. Start xovi (again, if the ink patch changed).
#  10. Install the Sudoku folder in AppLoad.
#
# Environment variables:
#   REMARKABLE_HOST     The address of the tablet on Wi-Fi, for example 192.168.1.20. Default (not
#                       set): 10.11.99.1, the USB connection. If the tablet does not answer on
#                       Wi-Fi, the installer asks: try again, or use the USB cable. Export the
#                       variable (`export REMARKABLE_HOST=...`), or set it before `bash`.
#   SUDOKU_VERSION      The version to install, for example v0.1.2 or the preview version pre-1a2b3c4
#                       (the name of a release on GitHub). Then the installer does not ask for the
#                       version. Default (empty): the installer asks.
#   SUDOKU_SSH_KEY      The SSH key of the installer. Default: ~/.ssh/id_ed25519_remarkable.
#   NO_COLOR            No colors. CLICOLOR_FORCE=1 gives colors also when the output is not a terminal.
#   SUDOKU_INSTALL_TTY  The input of the user. Default: /dev/tty. tests/install/run.sh gives a file.
#
# tests/install/fake/ssh simulates the tablet. It finds each remote command from its first word
# (": name;"). If you add or change a remote command, change the fake ssh also.
#
# This document is written in ASD-STE100 Simplified Technical English.

# Bash reads all of main before it starts. Thus `curl | bash` cannot run a part of the script.
main() {

set -u

REPO="https://github.com/garbles/sudoku"
# The version that the user selects (SUDOKU_VERSION). Empty: the installer asks.
WANT_VERSION="${SUDOKU_VERSION:-}"
# release.yml replaces these three values with the version, the checksum of its archive and the
# checksum of its ink patch. A copy from git installs the latest stable release.
SUDOKU_VERSION="@SUDOKU_VERSION@"
SUDOKU_SHA256="@SUDOKU_SHA256@"
PATCH_SHA256="@SUDOKU_PATCH_SHA256@"
# The bootstrap script of Vellum. Each Vellum release changes it, thus the installer does not keep a
# checksum. curl on the computer examines the TLS certificate of GitHub (the tablet does not), and the
# script examines the checksums of its own downloads.
VELLUM_BOOTSTRAP="https://github.com/vellum-dev/vellum-cli/releases/latest/download/bootstrap.sh"

HOST="${REMARKABLE_HOST:-10.11.99.1}"
KEY="${SUDOKU_SSH_KEY:-$HOME/.ssh/id_ed25519_remarkable}"
TTY="${SUDOKU_INSTALL_TTY:-/dev/tty}"
STEPS=10
WIDTH=66 # the width of the text in a box. All box text is ASCII.
UI_WARNED=0
PATCH_CHANGED=0
# The ink patch: AppLoad shows the ink of Sudoku with the fast pen waveform (no gaps). Each release
# has the patch as a separate file. The stamp file next to it gives the version that installed the
# patch and the checksum of the patch (the same file as src/update.rs writes).
PATCH_DIR=/home/root/xovi/exthome/qt-resource-rebuilder
PATCH_NAME=sudoku-penregion.qmd
PATCH_STAMP=sudoku-penregion.json

setup_colors
select_version
if ! exec 3<"$TTY"; then
  bad "The installer needs a terminal for your answers."
  exit 1
fi
WORK="$(mktemp -d "${TMPDIR:-/tmp}/sudoku-install.XXXXXX")" || exit 1
trap 'rm -rf "$WORK"' EXIT
MENU_CURSOR=0 # 1 while a menu hides the cursor of the terminal
MENU_STTY=""  # the terminal settings before the menu
trap 'cursor_on; printf "\n"; stop' INT
SSH_OPTS=(-o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new -o LogLevel=ERROR
  -o ServerAliveInterval=10 -o ServerAliveCountMax=3)

banner
step_tools
step_download
step_connect
step_key
step_vellum
step_appload
step_hashtable
step_patch
step_xovi
step_sudoku
finish
}

# --- Output ---------------------------------------------------------------------------------------

setup_colors() {
  R="" B="" GRAY="" WHITE="" CYAN="" GREEN="" YELLOW="" RED="" MAGENTA="" BLUE="" CODE="" BADGE=""
  if [ -z "${NO_COLOR:-}" ] && { [ -t 1 ] || [ "${CLICOLOR_FORCE:-0}" != 0 ]; }; then
    local e=$'\033['
    R="${e}0m" B="${e}1m" GRAY="${e}38;5;244m" WHITE="${e}1;38;5;255m" CYAN="${e}38;5;81m"
    GREEN="${e}38;5;114m" YELLOW="${e}38;5;221m" RED="${e}38;5;203m" MAGENTA="${e}38;5;177m"
    BLUE="${e}38;5;75m" CODE="${e}1;38;5;81m" BADGE="${e}1;38;5;16;48;5;81m"
  fi
}

ok() { printf '  %s✓%s %s\n' "$GREEN" "$R" "$*"; }
skip() { printf '  %s✓%s %s %s(done already)%s\n' "$GREEN" "$R" "$*" "$GRAY" "$R"; }
info() { printf '  %s•%s %s\n' "$BLUE" "$R" "$*"; }
act() { printf '  %s→%s %s\n' "$CYAN" "$R" "$*"; }
bad() { printf '  %s✗%s %s\n' "$RED" "$R" "$*"; }
warn() { printf '  %s!%s %s\n' "$YELLOW" "$R" "$*"; }

step() { printf '\n\n  %s %s/%s %s %s%s%s\n\n' "$BADGE" "$1" "$STEPS" "$R" "$B" "$2" "$R"; }

rep() { # text, count
  local s="" i=0
  while [ "$i" -lt "$2" ]; do s="$s$1"; i=$((i + 1)); done
  printf '%s' "$s"
}

# `code` and *bold* in box text.
markup() {
  if [ -n "$R" ]; then
    printf '%s' "$1" | sed -e "s/\`\([^\`]*\)\`/${CODE}\1${R}/g" -e "s/\*\([^*]*\)\*/${B}\1${R}/g"
  else
    printf '%s' "$1" | sed -e 's/[`*]//g'
  fi
}

box() { # color, title, lines (an empty line gives a blank line)
  local col="$1" title="$2" line plain
  shift 2
  printf '\n  %s╭─ %s%s%s%s %s╮%s\n' "$col" "$B" "$title" "$R" "$col" "$(rep ─ $((WIDTH - ${#title} - 1)))" "$R"
  for line in "$@"; do
    plain="$(printf '%s' "$line" | sed -e 's/[`*]//g')"
    printf '  %s│%s %s%*s %s│%s\n' "$col" "$R" "$(markup "$line")" $((WIDTH - ${#plain})) "" "$col" "$R"
  done
  printf '  %s╰%s╯%s\n' "$col" "$(rep ─ $((WIDTH + 2)))" "$R"
}

tilde() {
  case "$1" in
    "$HOME"/*) printf '~%s' "${1#"$HOME"}" ;;
    *) printf '%s' "$1" ;;
  esac
}

# One row of the small board in the banner. Tokens: Gd (starting number), Ud (your number),
# Hd (highlight), . (empty cell), | (line between two boxes).
grid_row() {
  local out="${GRAY}┃ " t
  for t in "$@"; do
    case "$t" in
      "|") out="$out${GRAY}│ " ;;
      .) out="$out${GRAY}· " ;;
      G?) out="$out${WHITE}${t#G}${R} " ;;
      U?) out="$out${CYAN}${t#U}${R} " ;;
      H?) out="$out${GREEN}${B}${t#H}${R} " ;;
    esac
  done
  printf '   %s%s┃%s' "$out" "$GRAY" "$R"
}

banner() {
  local version="$SUDOKU_VERSION"
  case "$version" in *@*) version="latest version" ;; pre-*) version="preview $version" ;; esac
  printf '\n   %s┏━━━━━━━┯━━━━━━━┯━━━━━━━┓%s\n' "$GRAY" "$R"
  printf '%s    %sS U D O K U%s\n' "$(grid_row G5 G3 . "|" . G7 . "|" . U1 G2)" "${B}${MAGENTA}" "$R"
  printf '%s    for reMarkable Paper Pro\n' "$(grid_row G6 . . "|" G1 G9 G5 "|" . H4 .)"
  printf '%s\n' "$(grid_row . G9 G8 "|" . . H4 "|" U3 G6 .)"
  printf '   %s┗━━━━━━━┷━━━━━━━┷━━━━━━━┛%s    %sInstaller · %s%s\n\n' "$GRAY" "$R" "$GRAY" "$version" "$R"
  printf '  This installer puts Sudoku on your reMarkable Paper Pro. At each step it\n'
  printf '  tells you what to do. It skips the steps that are done already.\n'
  printf '  %sTo stop at any time, push Ctrl+C.%s\n' "$GRAY" "$R"
}

# The user selects a version with SUDOKU_VERSION. A version without a prefix (0.1.2) gets the "v".
select_version() {
  case "$WANT_VERSION" in
    "") ;;
    *[!A-Za-z0-9._-]*)
      bad "The version $WANT_VERSION is not correct. Use only letters, digits, \".\", \"_\" and \"-\"."
      exit 1
      ;;
    [0-9]*) WANT_VERSION="v$WANT_VERSION" ;;
  esac
  if [ -n "$WANT_VERSION" ] && [ "$WANT_VERSION" != "$SUDOKU_VERSION" ]; then
    SUDOKU_VERSION="$WANT_VERSION"
    SUDOKU_SHA256="" # step_download reads the two checksums from version.json of the release.
    PATCH_SHA256=""
  fi
}

# --- Input ----------------------------------------------------------------------------------------

read_reply() {
  REPLY=""
  if ! IFS= read -r REPLY <&3; then
    printf '\n'
    REPLY=q
    return
  fi
  # A terminal shows what the user types. Show the answers from a file in the same way.
  [ "$TTY" = /dev/tty ] || printf '%s\n' "$REPLY"
}

pause() { # question
  printf '\n  %s?%s %s %s[Enter: continue, q: stop]%s ' "$MAGENTA" "$R" "$1" "$GRAY" "$R"
  read_reply
  case "$REPLY" in q | Q) stop ;; esac
}

confirm() { # question, default (y or n). Returns 0 for yes.
  local hint="[Y/n]"
  [ "$2" = n ] && hint="[y/N]"
  printf '\n  %s?%s %s %s%s%s ' "$MAGENTA" "$R" "$1" "$GRAY" "$hint" "$R"
  read_reply
  case "$REPLY" in
    y | Y | yes | Yes) return 0 ;;
    "") [ "$2" = y ] ;;
    *) return 1 ;;
  esac
}

# Shows the cursor again and restores the terminal settings after a menu.
cursor_on() {
  if [ "$MENU_CURSOR" = 1 ]; then
    printf '\033[?25h'
    [ -n "$MENU_STTY" ] && stty "$MENU_STTY" <&3 2>/dev/null
  fi
  MENU_CURSOR=0
}

# A menu. Each row is "label<TAB>detail". Sets CHOICE to the index of the selected row (0 is the
# first row). In a terminal, the user moves with the arrow keys (or j and k) and pushes Enter. With
# an input file (tests/install/run.sh), the menu shows numbers, and the user types a number.
MENU_SIZE=10 # the maximum number of rows on the screen. More rows scroll.
MENU_GAP=1   # 0: no blank line above the next question
choose() { # question, rows...
  local question="$1" pad=0 row label
  shift
  local rows=("$@")
  for row in "${rows[@]}"; do
    label="${row%%$'\t'*}"
    [ "${#label}" -gt "$pad" ] && pad="${#label}"
  done
  # Directly after the title of a step, the blank line is there already.
  MENU_TOP=4 # the lines above the rows: the blank line, the question and the blank line after it
  if [ "$MENU_GAP" = 1 ]; then printf '\n'; else MENU_TOP=3; fi
  MENU_GAP=1
  printf '  %s?%s %s%s%s\n\n' "$MAGENTA" "$R" "$B" "$question" "$R"
  if [ -t 3 ]; then
    menu_keys "$pad" "${rows[@]}"
  else
    menu_numbers "$pad" "${rows[@]}"
  fi
}

menu_row() { # selected (1 or 0), label width, number (empty in a terminal), row
  local label="${4%%$'\t'*}" detail="" num=""
  case "$4" in *$'\t'*) detail="${4#*$'\t'}" ;; esac
  [ -n "$3" ] && num="$(printf '%3s. ' "$3")"
  if [ "$1" = 1 ]; then
    printf '  %s❯%s %s%s%s%-*s%s  %s\n' "$CYAN" "$R" "$num" "$B" "$CYAN" "$2" "$label" "$R" "$detail"
  else
    printf '    %s%-*s  %s\n' "$num" "$2" "$label" "$detail"
  fi
}

menu_keys() { # label width, rows...
  local pad="$1" sel=0 top=0 key rest i n size hint
  shift
  n=$#
  size=$n
  [ "$size" -gt "$MENU_SIZE" ] && size="$MENU_SIZE"
  local rows=("$@")
  # No echo during all of the menu: a key that comes between two reads must not show (for example
  # when the user holds an arrow key).
  MENU_STTY="$(stty -g <&3 2>/dev/null)"
  stty -echo -icanon <&3 2>/dev/null
  printf '\033[?25l'
  MENU_CURSOR=1
  while :; do
    for ((i = top; i < top + size; i++)); do
      printf '\033[2K'
      menu_row "$([ "$i" = "$sel" ] && echo 1 || echo 0)" "$pad" "" "${rows[$i]}"
    done
    hint="↑↓ move · Enter select · q stop"
    [ "$n" -gt "$size" ] && hint="$hint · $((sel + 1)) of $n"
    printf '\033[2K\n\033[2K    %s%s%s\n' "$GRAY" "$hint" "$R"
    IFS= read -rsn1 key <&3 || key=q
    case "$key" in
      $'\033')
        rest=""
        IFS= read -rsn2 -t 1 rest <&3
        case "$rest" in
          "[A" | OA) key=k ;;
          "[B" | OB) key=j ;;
          *) key="" ;;
        esac
        [ -z "$key" ] && key=none
        ;;
    esac
    case "$key" in
      k) [ "$sel" -gt 0 ] && sel=$((sel - 1)) ;;
      j) [ "$sel" -lt $((n - 1)) ] && sel=$((sel + 1)) ;;
      "") break ;;
      q | Q)
        cursor_on
        stop
        ;;
    esac
    [ "$sel" -lt "$top" ] && top="$sel"
    [ "$sel" -ge $((top + size)) ] && top=$((sel - size + 1))
    printf '\033[%dA' $((size + 2))
  done
  # Remove the menu and the question. The caller shows the selection.
  printf '\033[%dA\033[J' $((size + MENU_TOP + 1))
  cursor_on
  CHOICE="$sel"
}

menu_numbers() { # label width, rows...
  local pad="$1" i=0 row
  shift
  for row in "$@"; do
    menu_row "$([ "$i" = 0 ] && echo 1 || echo 0)" "$pad" "$((i + 1))" "$row"
    i=$((i + 1))
  done
  while :; do
    printf '\n  %s?%s Type a number. %s[Enter: 1, q: stop]%s ' "$MAGENTA" "$R" "$GRAY" "$R"
    read_reply
    case "$REPLY" in
      "") CHOICE=0 ;;
      q | Q) stop ;;
      *[!0-9]* | 0) CHOICE=-1 ;;
      *) CHOICE=$((REPLY - 1)) ;;
    esac
    [ "$CHOICE" -ge 0 ] && [ "$CHOICE" -lt "$#" ] && return
    bad "Type a number from 1 to $#."
  done
}

stop() {
  printf '\n  %sThe installer stopped.%s Run the same command again to continue.\n' "$YELLOW" "$R"
  printf '  The installer skips the steps that are done.\n\n'
  exit 1
}

# --- SSH ------------------------------------------------------------------------------------------

# The installer key only. -n is not sufficient: `curl | bash` gives the script on stdin.
ssh_key() { ssh "${SSH_OPTS[@]}" -o BatchMode=yes -o IdentitiesOnly=yes -i "$KEY" "root@$HOST" "$1"; }
remote() { ssh_key "$1" </dev/null; }

# Sets PROBE to ok, auth (the tablet answers, but not to the key), refused (SSH is off), hostkey or
# none (no answer).
probe() {
  local err
  err="$(ssh_key ': probe; true' 2>&1 >/dev/null </dev/null)"
  if [ $? = 0 ]; then
    PROBE=ok
    return
  fi
  case "$err" in
    *"Host key verification failed"* | *"IDENTIFICATION HAS CHANGED"*) PROBE=hostkey ;;
    *"Permission denied"*) PROBE=auth ;;
    *"Connection refused"*) PROBE=refused ;;
    *) PROBE=none ;;
  esac
}

lost() {
  bad "The connection to the tablet stopped."
  box "$RED" "What to do" \
    "1. Make sure that the USB cable is connected and the tablet is on." \
    "2. Run the installer again. It continues at this step."
  printf '\n'
  exit 1
}

# Runs the remote condition $2. Returns 0 if it is true and 1 if it is false.
check() { # name, condition
  local out
  out="$(remote ": $1; if $2; then echo yes; else echo no; fi" 2>/dev/null)"
  case "$out" in
    yes) return 0 ;;
    no) return 1 ;;
    *) lost ;;
  esac
}

# Runs a remote command and shows its output in a frame. Returns the exit code of the command.
stream() { # command, input file (default: no input)
  local line
  ssh_key "$1" <"${2:-/dev/null}" 2>&1 | while IFS= read -r line; do
    line="${line%$'\r'}"
    case "$line" in *"exec bash --login"*) continue ;; esac # Vellum: not necessary here
    printf '    %s│ %s%s\n' "$GRAY" "$line" "$R"
  done
  return "${PIPESTATUS[0]}"
}

sha_of() { "${SHA[@]}" "$1" | cut -d ' ' -f 1; }

# --- Steps ----------------------------------------------------------------------------------------

step_tools() {
  step 1 "Programs on this computer"
  local missing="" t
  for t in ssh ssh-keygen curl tar; do
    command -v "$t" >/dev/null 2>&1 || missing="$missing $t"
  done
  if command -v sha256sum >/dev/null 2>&1; then
    SHA=(sha256sum)
  elif command -v shasum >/dev/null 2>&1; then
    SHA=(shasum -a 256)
  else
    missing="$missing sha256sum"
  fi
  if [ -n "$missing" ]; then
    bad "These programs are missing:$missing"
    box "$RED" "What to do" \
      "Install the missing programs. Then run the installer again." \
      "" \
      "Debian or Ubuntu:  \`sudo apt install openssh-client curl tar\`" \
      "Fedora:            \`sudo dnf install openssh-clients curl tar\`" \
      "macOS:             these programs are part of macOS."
    printf '\n'
    exit 1
  fi
  ok "ssh, ssh-keygen, curl and tar are available."
}

# Downloads a file. If the network fails, the user can try again. With "soft", returns 1 if the
# server answers with an error (for example 404: no such file).
fetch() { # url, file, [soft]
  local code
  while :; do
    curl -fsSL -o "$2" "$1" 2>"$WORK/curl.err"
    code=$?
    [ "$code" = 0 ] && return 0
    [ "$code" = 22 ] && [ "${3:-}" = soft ] && return 1
    bad "The download failed: $(tail -n 1 "$WORK/curl.err")"
    box "$YELLOW" "What to do" \
      "Make sure that this computer is connected to the internet." \
      "" \
      "The file: $(basename "$1")"
    pause "Try again?"
  done
}

json_value() { sed -n 's/.*"'"$1"'": *"\([^"]*\)".*/\1/p' "$WORK/version.json"; }

# The releases of Sudoku on GitHub, the newest first. Sets LIST to rows for choose and TAGS to the
# names of the releases. Returns 1 if GitHub does not give the list.
RELEASES_API="https://api.github.com/repos/garbles/sudoku/releases?per_page=100"
list_releases() {
  local tag="" pre="" when kind latest=""
  LIST=() TAGS=()
  fetch "$RELEASES_API" "$WORK/releases.json" soft || return 1
  # grep -o finds the three values in compact and in indented JSON. Text in the release notes has
  # \" in place of ", thus it does not agree with the pattern.
  while IFS= read -r line; do
    case "$line" in
      '"tag_name"'*) tag="$(printf '%s' "$line" | sed 's/.*: *"\(.*\)"/\1/')" ;;
      '"prerelease"'*) pre="${line##* }" pre="${pre##*:}" ;;
      '"published_at"'*)
        [ -n "$tag" ] || continue
        when="$(printf '%s' "$line" | sed 's/.*: *"\([0-9-]*\)T\([0-9]*:[0-9]*\).*/\1 \2/')"
        if [ "$pre" = true ]; then
          kind="${YELLOW}preview${R}"
        elif [ -z "$latest" ]; then
          kind="${GREEN}latest ${R}"
          latest="$tag"
        else
          kind="stable "
        fi
        TAGS+=("$tag")
        LIST+=("$tag"$'\t'"$kind  $GRAY$when$R")
        tag="" pre=""
        ;;
    esac
  done < <(grep -oE '"(tag_name|prerelease|published_at)": *("[^"]*"|true|false)' "$WORK/releases.json")
  [ "${#TAGS[@]}" -gt 0 ]
}

# A copy from git has no version. Then version.json of the latest release gives it.
latest_version() {
  case "$SUDOKU_VERSION" in
    *@*)
      act "Looking for the latest version of Sudoku..."
      fetch "$REPO/releases/latest/download/version.json" "$WORK/version.json"
      SUDOKU_VERSION="$(json_value version)"
      SUDOKU_SHA256="$(json_value sha256)"
      PATCH_SHA256="$(json_value patch_sha256)"
      ;;
  esac
}

# Asks for the version. Sets SUDOKU_VERSION, and SUDOKU_SHA256 if the user selects the latest version.
ask_version() {
  case "$SUDOKU_VERSION" in *@*) latest_version ;; *) MENU_GAP=0 ;; esac
  choose "Which version of Sudoku do you want to install?" \
    "The latest version"$'\t'"$SUDOKU_VERSION" \
    "A different version"$'\t'"${GRAY}a preview version or an earlier version${R}"
  if [ "$CHOICE" = 0 ]; then
    ok "Version: the latest version ($SUDOKU_VERSION)."
    return
  fi
  act "Looking for the versions of Sudoku..."
  if list_releases; then
    choose "Select a version. The newest is at the top." "${LIST[@]}"
    SUDOKU_VERSION="${TAGS[$CHOICE]}"
  else
    bad "GitHub did not give the list of versions."
    info "The versions are also at $REPO/releases"
    printf '\n  %s?%s Type the name of the version, for example pre-1a2b3c4. %s[q: stop]%s ' "$MAGENTA" "$R" "$GRAY" "$R"
    read_reply
    case "$REPLY" in "" | q | Q) stop ;; esac
    WANT_VERSION="$REPLY"
    select_version
  fi
  SUDOKU_SHA256="" PATCH_SHA256=""
  ok "Version: $SUDOKU_VERSION."
}

step_download() {
  step 2 "Download Sudoku"
  [ -n "$WANT_VERSION" ] || ask_version
  if [ -z "$SUDOKU_SHA256" ]; then
    act "Looking for Sudoku $SUDOKU_VERSION..."
    if ! fetch "$REPO/releases/download/$SUDOKU_VERSION/version.json" "$WORK/version.json" soft; then
      bad "Sudoku $SUDOKU_VERSION is not available."
      box "$RED" "What to do" \
        "1. Make sure that the version is correct, for example \`v0.1.2\` or" \
        "   \`pre-1a2b3c4\`. The releases are on GitHub:" \
        "   $REPO/releases" \
        "2. To install the latest version, run the installer again and" \
        "   select the latest version."
      printf '\n'
      exit 1
    fi
    if [ "$(json_value version)" != "$SUDOKU_VERSION" ]; then
      bad "The release $SUDOKU_VERSION has an incorrect version.json."
      exit 1
    fi
    SUDOKU_SHA256="$(json_value sha256)"
    PATCH_SHA256="$(json_value patch_sha256)"
  fi
  case "$SUDOKU_VERSION" in
    pre-*) warn "$SUDOKU_VERSION is a preview version. It can have errors." ;;
  esac
  act "Downloading Sudoku $SUDOKU_VERSION from GitHub..."
  fetch "$REPO/releases/download/$SUDOKU_VERSION/sudoku-$SUDOKU_VERSION.tar.gz" "$WORK/sudoku.tar.gz"
  [ "$(sha_of "$WORK/sudoku.tar.gz")" = "$SUDOKU_SHA256" ] || bad_checksum
  # A release before the ink patch was a separate file has no patch_sha256. Then step 8 does nothing.
  if [ -n "$PATCH_SHA256" ]; then
    act "Downloading the ink patch..."
    fetch "$REPO/releases/download/$SUDOKU_VERSION/$PATCH_NAME" "$WORK/$PATCH_NAME"
    [ "$(sha_of "$WORK/$PATCH_NAME")" = "$PATCH_SHA256" ] || bad_checksum
  fi
  mkdir "$WORK/app" && tar -xzf "$WORK/sudoku.tar.gz" -C "$WORK/app" 2>/dev/null
  if [ ! -f "$WORK/app/sudoku/sudoku" ]; then
    bad "The archive does not contain the app."
    exit 1
  fi
  APP_HASH="$(sha_of "$WORK/app/sudoku/sudoku")"
  if [ -n "$PATCH_SHA256" ]; then
    ok "Downloaded Sudoku $SUDOKU_VERSION and the ink patch. The checksums are correct."
  else
    ok "Downloaded Sudoku $SUDOKU_VERSION. The checksum is correct."
  fi
}

bad_checksum() {
  bad "The checksum of the download is not correct."
  box "$RED" "What to do" \
    "Do not install this file. Run the installer again later." \
    "If the problem continues, tell the developer:" \
    "$REPO/issues"
  printf '\n'
  exit 1
}

step_connect() {
  step 3 "Connect the tablet"
  if [ -n "${REMARKABLE_HOST:-}" ]; then
    info "REMARKABLE_HOST is set: the installer connects to $HOST."
  else
    info "REMARKABLE_HOST is not set: the installer uses the USB cable."
  fi
  act "Looking for the tablet at $HOST..."
  while :; do
    probe
    case "$PROBE" in
      ok | auth)
        ok "Found the tablet at $HOST."
        return
        ;;
      refused)
        bad "The tablet is connected, but SSH is off."
        box "$YELLOW" "Turn on developer mode" \
          "SSH operates only in developer mode." \
          "" \
          "*CAUTION: Developer mode erases all data on the tablet.*" \
          "Before you continue, sync your notebooks to the reMarkable cloud." \
          "" \
          "1. On the tablet, open *Settings*." \
          "2. Go to *General* > *Paper Tablet* > *Software* > *Advanced*." \
          "3. Tap *Developer mode*. Do the instructions on the tablet." \
          "4. After the restart, set up the tablet again." \
          "5. Connect the USB cable again and unlock the tablet."
        pause "Is developer mode on?"
        ;;
      hostkey)
        warn "The tablet has a new SSH identity (host key)."
        box "$YELLOW" "Why this occurs" \
          "A factory reset gives the tablet a new identity. Developer mode" \
          "does a factory reset. Your computer knows the old identity, and" \
          "SSH does not connect." \
          "" \
          "The installer can remove the old identity from" \
          "\`~/.ssh/known_hosts\`. Then SSH accepts the new identity."
        confirm "Remove the old identity of $HOST?" y || stop
        ssh-keygen -R "$HOST" >/dev/null 2>&1
        ok "Removed the old identity."
        ;;
      *)
        bad "No answer from the tablet at $HOST."
        if [ "$HOST" = 10.11.99.1 ]; then
          box "$YELLOW" "Connect the tablet" \
            "1. Connect the tablet to this computer with a USB-C cable." \
            "   Use a cable for data. Some cables only charge." \
            "2. Wake the tablet and unlock it." \
            "3. Wait approximately 10 seconds." \
            "" \
            "Developer mode must be on. If it is off, turn it on first:" \
            "*Settings* > *General* > *Paper Tablet* > *Software* > *Advanced*" \
            "> *Developer mode*. CAUTION: this erases all data on the tablet."
          pause "Is the tablet connected?"
        else
          box "$YELLOW" "Connect the tablet on Wi-Fi" \
            "The installer uses the address $HOST (REMARKABLE_HOST)." \
            "" \
            "1. Make sure that the tablet is on, unlocked and in the same" \
            "   Wi-Fi network as this computer." \
            "2. SSH on Wi-Fi is off by default. To turn it on, connect" \
            "   with USB first and run \`rm-ssh-over-wlan on\` on the tablet." \
            "" \
            "You can also use the USB cable."
          choose "What do you want to do?" \
            "Try again"$'\t'"${GRAY}on Wi-Fi at $HOST${R}" \
            "Use the USB cable"$'\t'"${GRAY}at 10.11.99.1${R}"
          if [ "$CHOICE" = 1 ]; then
            HOST=10.11.99.1
            ok "The installer uses the USB cable."
            info "Connect the tablet to this computer with a USB-C cable and unlock it."
            act "Looking for the tablet at $HOST..."
            continue
          fi
          ok "Try again on Wi-Fi."
        fi
        ;;
    esac
    act "Looking for the tablet at $HOST again..."
  done
}

# The remote command reads the public key from stdin and adds it to authorized_keys one time.
AUTHORIZE=': authorize; mkdir -p /home/root/.ssh && chmod 700 /home/root/.ssh && touch /home/root/.ssh/authorized_keys && chmod 600 /home/root/.ssh/authorized_keys && read -r k && { grep -qxF "$k" /home/root/.ssh/authorized_keys || echo "$k" >> /home/root/.ssh/authorized_keys; }'

step_key() {
  step 4 "SSH key"
  if [ -f "$KEY" ]; then
    skip "This computer has the SSH key $(tilde "$KEY")."
  else
    act "Making an SSH key for the tablet..."
    mkdir -p -m 700 "$(dirname "$KEY")"
    if ! ssh-keygen -q -t ed25519 -N "" -C "sudoku-installer" -f "$KEY" </dev/null; then
      bad "ssh-keygen could not make the key."
      exit 1
    fi
    ok "Made the SSH key $(tilde "$KEY")."
  fi
  probe
  if [ "$PROBE" = ok ]; then
    skip "The tablet accepts the SSH key."
  else
    authorize
  fi
  tablet_info
}

authorize() {
  # A different key of this computer can open the tablet already. Then the password is not necessary.
  if ssh "${SSH_OPTS[@]}" -o BatchMode=yes "root@$HOST" "$AUTHORIZE" <"$KEY.pub" >/dev/null 2>&1; then
    probe
    if [ "$PROBE" = ok ]; then
      ok "Added the key to the tablet with your other SSH key."
      return
    fi
  fi
  info "The tablet does not know the key yet. To add it, the installer needs the"
  info "password of the tablet one time."
  box "$CYAN" "Find the SSH password" \
    "1. On the tablet, open *Settings*." \
    "2. Go to *General* > *Help* > *About* > *Copyrights and licenses*." \
    "3. Under *General information*, find the password for \`root\`." \
    "   Each tablet has a different password." \
    "4. Type the password below. Then push Enter." \
    "" \
    "The password does not show while you type. This is normal."
  while :; do
    printf '\n'
    if ssh "${SSH_OPTS[@]}" -o PubkeyAuthentication=no -o PreferredAuthentications=keyboard-interactive,password \
      -o NumberOfPasswordPrompts=3 "root@$HOST" "$AUTHORIZE" <"$KEY.pub"; then
      probe
      if [ "$PROBE" = ok ]; then
        ok "The tablet accepts the SSH key. You do not need the password again."
        return
      fi
      bad "The tablet did not accept the SSH key."
      exit 1
    fi
    bad "The tablet did not accept the password."
    box "$YELLOW" "What to do" \
      "1. Find the password again (see above)." \
      "2. Type it exactly. Upper and lower case letters are different." \
      "" \
      "After a factory reset, the tablet has a new password."
    pause "Try again?"
  done
}

tablet_info() {
  local out model os
  out="$(remote ': info; m=$(cat /sys/devices/soc0/machine 2>/dev/null); v=$(sed -n "s/^REMARKABLE_RELEASE_VERSION=//p" /usr/share/remarkable/update.conf 2>/dev/null); echo "$m"; echo "$v"')" || lost
  model="$(printf '%s\n' "$out" | sed -n 1p)"
  os="$(printf '%s\n' "$out" | sed -n 2p)"
  # The device tree gives the code name: Ferrari is the Paper Pro, Chiappa is the Paper Pro Move.
  case "$model" in
    *Ferrari*)
      ok "Tablet: reMarkable Paper Pro, software ${os:-unknown}."
      ;;
    "")
      warn "Tablet: unknown model, software ${os:-unknown}. The installer continues."
      ;;
    *)
      case "$model" in *Chiappa*) model="reMarkable Paper Pro Move" ;; esac
      warn "Tablet: $model, software ${os:-unknown}."
      box "$YELLOW" "This tablet is not a reMarkable Paper Pro" \
        "Sudoku is made for the screen of the Paper Pro (1620 x 2160)." \
        "On a different tablet, it can show incorrectly or not start."
      confirm "Continue?" n || stop
      ;;
  esac
}

need_internet() {
  while ! check has-internet 'wget -q -T 10 --no-check-certificate -O /dev/null https://github.com >/dev/null 2>&1'; do
    bad "The tablet is not connected to the internet."
    box "$YELLOW" "Connect the tablet to Wi-Fi" \
      "The tablet downloads the next part from the internet." \
      "" \
      "1. On the tablet, open *Settings* > *Wi-Fi*." \
      "2. Turn on Wi-Fi and connect to your network." \
      "3. Keep the USB cable connected."
    pause "Is the tablet connected to Wi-Fi?"
  done
}

failed() { # what
  bad "$1"
  box "$RED" "What to do" \
    "1. Read the messages above." \
    "2. Make sure that the tablet is connected to Wi-Fi." \
    "3. Run the installer again. It continues at this step." \
    "" \
    "Help for Vellum, AppLoad and xovi:" \
    "https://github.com/vellum-dev/vellum"
  printf '\n'
  exit 1
}

step_vellum() {
  step 5 "Vellum (package manager)"
  if check has-vellum '[ -x /home/root/.vellum/bin/vellum ]'; then
    skip "Vellum is installed."
    return
  fi
  info "Vellum installs AppLoad and xovi on the tablet."
  need_internet
  act "Downloading the Vellum installer..."
  fetch "$VELLUM_BOOTSTRAP" "$WORK/vellum-bootstrap.sh"
  act "Installing Vellum on the tablet..."
  stream ': install-vellum; f=/tmp/vellum-bootstrap.sh; cat > $f && bash $f; s=$?; rm -f $f; exit $s' "$WORK/vellum-bootstrap.sh" ||
    failed "Vellum did not install."
  check has-vellum '[ -x /home/root/.vellum/bin/vellum ]' || failed "Vellum did not install."
  ok "Vellum is installed."
}

step_appload() {
  step 6 "AppLoad (app launcher)"
  if check has-appload '[ -f /home/root/xovi/extensions.d/appload.so ]'; then
    skip "AppLoad is installed."
    return
  fi
  info "AppLoad starts apps like Sudoku. It needs xovi. Vellum installs the two."
  need_internet
  act "Installing AppLoad with Vellum..."
  stream ': install-appload; /home/root/.vellum/bin/vellum update && /home/root/.vellum/bin/vellum add appload' ||
    failed "Vellum did not install AppLoad."
  check has-appload '[ -f /home/root/xovi/extensions.d/appload.so ]' || failed "Vellum did not install AppLoad."
  ok "AppLoad is installed."
}

step_hashtable() {
  step 7 "xovi hashtable"
  if check has-hashtab '[ -s /home/root/xovi/exthome/qt-resource-rebuilder/hashtab ]'; then
    skip "The hashtable is available."
    return
  fi
  info "xovi needs a table of the user interface files of the tablet."
  box "$YELLOW" "Before you continue" \
    "The installer stops the user interface of the tablet for 1 to 2" \
    "minutes. During this time, the screen does not change. This is" \
    "normal." \
    "" \
    "1. Close all notebooks on the tablet." \
    "2. If the tablet asks for its passcode, type it on the tablet."
  pause "Ready?"
  UI_WARNED=1
  act "Making the hashtable. Wait 1 to 2 minutes..."
  stream ': make-hashtab; printf "\n" | /home/root/xovi/rebuild_hashtable > /tmp/sudoku-hashtab.log 2>&1; [ -s /home/root/xovi/exthome/qt-resource-rebuilder/hashtab ] || { tail -n 20 /tmp/sudoku-hashtab.log; exit 1; }' ||
    failed "xovi could not make the hashtable."
  ok "The hashtable is available."
}

# Sets HAVE_PATCH to the checksum of the installed patch and HAVE_STAMP to the checksum in its stamp
# file. Each is empty if the file is missing.
patch_state() {
  local out
  out="$(remote ': patch-hash; d='"$PATCH_DIR"'; echo "patch=$(sha256sum "$d/'"$PATCH_NAME"'" 2>/dev/null | cut -d " " -f 1)"; echo "stamp=$(sed -n "s/.*\"sha256\": *\"\([0-9a-f]*\)\".*/\1/p" "$d/'"$PATCH_STAMP"'" 2>/dev/null)"')" || lost
  HAVE_PATCH="$(printf '%s\n' "$out" | sed -n 's/^patch=//p')"
  HAVE_STAMP="$(printf '%s\n' "$out" | sed -n 's/^stamp=//p')"
}

step_patch() {
  step 8 "Ink patch"
  local file="$WORK/$PATCH_NAME" want="$PATCH_SHA256"
  if [ -z "$want" ]; then
    info "This version of Sudoku has no ink patch."
    return
  fi
  patch_state
  if [ "$HAVE_PATCH" = "$want" ] && [ "$HAVE_STAMP" = "$want" ]; then
    skip "The ink patch is installed."
    return
  fi
  if [ "$HAVE_PATCH" = "$want" ]; then
    act "Writing the stamp file of the ink patch..."
  else
    info "With the ink patch, the ink has no gaps while you write."
    info "xovi applies the patch when it starts."
    act "Copying the ink patch to the tablet..."
  fi
  # The tablet examines the checksum before it replaces the patch. Then it writes the stamp file.
  ssh_key ': install-patch; set -e; d='"$PATCH_DIR"'; mkdir -p "$d"; cat > "$d/'"$PATCH_NAME"'.new"; [ "$(sha256sum "$d/'"$PATCH_NAME"'.new" | cut -d " " -f 1)" = '"$want"' ] || { rm -f "$d/'"$PATCH_NAME"'.new"; exit 1; }; mv "$d/'"$PATCH_NAME"'.new" "$d/'"$PATCH_NAME"'"; printf "{\"version\": \"%s\", \"sha256\": \"%s\"}\n" '"$SUDOKU_VERSION $want"' > "$d/'"$PATCH_STAMP"'.new"; mv "$d/'"$PATCH_STAMP"'.new" "$d/'"$PATCH_STAMP"'"' \
    <"$file" || failed "The installer could not copy the ink patch to the tablet."
  [ "$HAVE_PATCH" = "$want" ] || PATCH_CHANGED=1
  patch_state
  [ "$HAVE_PATCH" = "$want" ] && [ "$HAVE_STAMP" = "$want" ] || failed "The installed copy of the ink patch is not correct."
  ok "The ink patch is installed."
}

step_xovi() {
  step 9 "Start xovi"
  if check has-xovi 'p=$(pidof xochitl) && grep -q xovi.so /proc/${p%% *}/maps 2>/dev/null'; then
    if [ "$PATCH_CHANGED" = 0 ]; then
      skip "xovi is on."
      return
    fi
    info "xovi must start again to apply the ink patch."
  else
    info "xovi adds AppLoad to the tablet. Each restart of the tablet stops xovi."
  fi
  if [ "$UI_WARNED" = 0 ]; then
    box "$YELLOW" "Before you continue" \
      "The user interface of the tablet starts again. This takes some" \
      "seconds. Close all notebooks on the tablet."
    pause "Ready?"
  fi
  act "Starting xovi..."
  stream ': start-xovi; /home/root/xovi/start > /tmp/sudoku-xovi.log 2>&1; i=0; while [ $i -lt 30 ]; do p=$(pidof xochitl) && grep -q xovi.so /proc/${p%% *}/maps 2>/dev/null && exit 0; sleep 1; i=$((i + 1)); done; tail -n 20 /tmp/sudoku-xovi.log; exit 1' ||
    failed "xovi did not start."
  ok "xovi is on."
}

step_sudoku() {
  step 10 "Sudoku $SUDOKU_VERSION"
  local installed
  installed="$(remote ': app-hash; sha256sum /home/root/xovi/exthome/appload/sudoku/sudoku 2>/dev/null | cut -d " " -f 1')" || lost
  if [ "$installed" = "$APP_HASH" ]; then
    skip "Sudoku $SUDOKU_VERSION is installed."
    return
  fi
  if [ -n "$installed" ]; then
    info "A different version of Sudoku is on the tablet. The installer replaces it."
    info "Your games, settings and statistics stay on the tablet."
  fi
  act "Copying Sudoku to the tablet..."
  # The new folder replaces the old folder only after all checks.
  if ! ssh_key ': install-app; set -e; d=/home/root/xovi/exthome/appload; t=$d/.sudoku-install; rm -rf "$t"; mkdir -p "$t"; cat > "$t/sudoku.tar.gz"; [ "$(sha256sum "$t/sudoku.tar.gz" | cut -d " " -f 1)" = '"$SUDOKU_SHA256"' ]; tar -xzf "$t/sudoku.tar.gz" -C "$t"; chmod 755 "$t/sudoku/sudoku"; rm -rf "$d/sudoku.old"; if [ -d "$d/sudoku" ]; then mv "$d/sudoku" "$d/sudoku.old"; fi; mv "$t/sudoku" "$d/sudoku"; rm -rf "$d/sudoku.old" "$t"' \
    <"$WORK/sudoku.tar.gz"; then
    failed "The installer could not copy Sudoku to the tablet."
  fi
  installed="$(remote ': app-hash; sha256sum /home/root/xovi/exthome/appload/sudoku/sudoku 2>/dev/null | cut -d " " -f 1')" || lost
  [ "$installed" = "$APP_HASH" ] || failed "The installed copy of Sudoku is not correct."
  ok "Sudoku $SUDOKU_VERSION is installed."
}

finish() {
  box "$GREEN" "Sudoku is ready" \
    "1. On the tablet, open the menu at the left side." \
    "2. Tap *AppLoad*. Then tap *Sudoku*." \
    "" \
    "If Sudoku was open, close it and open it again." \
    "" \
    "Each restart of the tablet stops xovi. Then AppLoad is not in the" \
    "menu. To start xovi again, run this installer again. It skips the" \
    "other steps."
  printf '\n  %sTo run the installer again:%s\n' "$GRAY" "$R"
  printf '  %scurl -fsSL %s/releases/latest/download/install.sh | bash%s\n\n' "$CODE" "$REPO" "$R"
}

main "$@"
