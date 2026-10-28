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
#   2. Download the release archive and examine its SHA-256 checksum.
#   3. Find the tablet (USB cable, developer mode).
#   4. Make an SSH key and add it to the tablet. The user types the root password one time.
#   5. Install Vellum, the package manager.
#   6. Install AppLoad (with xovi) with Vellum.
#   7. Make the hashtable of xovi.
#   8. Start xovi.
#   9. Install the Sudoku folder in AppLoad.
#
# Environment variables:
#   REMARKABLE_HOST     The address of the tablet. Default: 10.11.99.1 (the USB connection).
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
# release.yml replaces these two values with the version and the checksum of its archive. A copy
# from git installs the latest stable release.
SUDOKU_VERSION="@SUDOKU_VERSION@"
SUDOKU_SHA256="@SUDOKU_SHA256@"
# The bootstrap script of Vellum. Each Vellum release changes it, thus the installer does not keep a
# checksum. curl on the computer examines the TLS certificate of GitHub (the tablet does not), and the
# script examines the checksums of its own downloads.
VELLUM_BOOTSTRAP="https://github.com/vellum-dev/vellum-cli/releases/latest/download/bootstrap.sh"

HOST="${REMARKABLE_HOST:-10.11.99.1}"
KEY="${SUDOKU_SSH_KEY:-$HOME/.ssh/id_ed25519_remarkable}"
TTY="${SUDOKU_INSTALL_TTY:-/dev/tty}"
STEPS=9
WIDTH=66 # the width of the text in a box. All box text is ASCII.
UI_WARNED=0

setup_colors
if ! exec 3<"$TTY"; then
  bad "The installer needs a terminal for your answers."
  exit 1
fi
WORK="$(mktemp -d "${TMPDIR:-/tmp}/sudoku-install.XXXXXX")" || exit 1
trap 'rm -rf "$WORK"' EXIT
trap 'printf "\n"; stop' INT
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
  case "$version" in *@*) version="latest version" ;; esac
  printf '\n   %s┏━━━━━━━┯━━━━━━━┯━━━━━━━┓%s\n' "$GRAY" "$R"
  printf '%s    %sS U D O K U%s\n' "$(grid_row G5 G3 . "|" . G7 . "|" . U1 G2)" "${B}${MAGENTA}" "$R"
  printf '%s    for reMarkable Paper Pro\n' "$(grid_row G6 . . "|" G1 G9 G5 "|" . H4 .)"
  printf '%s\n' "$(grid_row . G9 G8 "|" . . H4 "|" U3 G6 .)"
  printf '   %s┗━━━━━━━┷━━━━━━━┷━━━━━━━┛%s    %sInstaller · %s%s\n\n' "$GRAY" "$R" "$GRAY" "$version" "$R"
  printf '  This installer puts Sudoku on your reMarkable Paper Pro. At each step it\n'
  printf '  tells you what to do. It skips the steps that are done already.\n'
  printf '  %sTo stop at any time, push Ctrl+C.%s\n' "$GRAY" "$R"
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

fetch() { # url, file
  while ! curl -fsSL -o "$2" "$1" 2>"$WORK/curl.err"; do
    bad "The download failed: $(tail -n 1 "$WORK/curl.err")"
    box "$YELLOW" "What to do" \
      "Make sure that this computer is connected to the internet." \
      "" \
      "The file: $(basename "$1")"
    pause "Try again?"
  done
}

step_download() {
  step 2 "Download Sudoku"
  case "$SUDOKU_VERSION" in
    *@*)
      act "Looking for the latest version of Sudoku..."
      fetch "$REPO/releases/latest/download/version.json" "$WORK/version.json"
      SUDOKU_VERSION="$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$WORK/version.json")"
      SUDOKU_SHA256="$(sed -n 's/.*"sha256": *"\([^"]*\)".*/\1/p' "$WORK/version.json")"
      ;;
  esac
  act "Downloading Sudoku $SUDOKU_VERSION from GitHub..."
  fetch "$REPO/releases/download/$SUDOKU_VERSION/sudoku-$SUDOKU_VERSION.tar.gz" "$WORK/sudoku.tar.gz"
  if [ "$(sha_of "$WORK/sudoku.tar.gz")" != "$SUDOKU_SHA256" ]; then
    bad "The checksum of the download is not correct."
    box "$RED" "What to do" \
      "Do not install this file. Run the installer again later." \
      "If the problem continues, tell the developer:" \
      "$REPO/issues"
    printf '\n'
    exit 1
  fi
  mkdir "$WORK/app" && tar -xzf "$WORK/sudoku.tar.gz" -C "$WORK/app" 2>/dev/null
  if [ ! -f "$WORK/app/sudoku/sudoku" ]; then
    bad "The archive does not contain the app."
    exit 1
  fi
  APP_HASH="$(sha_of "$WORK/app/sudoku/sudoku")"
  ok "Downloaded Sudoku $SUDOKU_VERSION. The checksum is correct."
}

step_connect() {
  step 3 "Connect the tablet"
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
        else
          box "$YELLOW" "Connect the tablet" \
            "The installer uses the address $HOST (REMARKABLE_HOST)." \
            "" \
            "1. Make sure that the tablet is on and in the same network." \
            "2. SSH on Wi-Fi is off by default. To turn it on, connect" \
            "   with USB first and run \`rm-ssh-over-wlan on\` on the tablet." \
            "" \
            "To use the USB cable, run the installer without REMARKABLE_HOST."
        fi
        pause "Is the tablet connected?"
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

step_xovi() {
  step 8 "Start xovi"
  if check has-xovi 'p=$(pidof xochitl) && grep -q xovi.so /proc/${p%% *}/maps 2>/dev/null'; then
    skip "xovi is on."
    return
  fi
  info "xovi adds AppLoad to the tablet. Each restart of the tablet stops xovi."
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
  step 9 "Sudoku $SUDOKU_VERSION"
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
