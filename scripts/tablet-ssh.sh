#!/bin/sh
# Run a command on the tablet with the SSH key of the installer (install.sh).
#
# Usage: scripts/tablet-ssh.sh HOST 'COMMAND'
#
# The script sends its standard input to the command, thus a pipe also operates:
#   tar -czf - sudoku | scripts/tablet-ssh.sh HOST 'tar -xzf - -C /tmp'
#
# Use this script in the skills, and do not put the SSH options in a shell variable: zsh does not
# split a variable into words. The Makefile has the same options (SSH).
#
# accept-new is necessary: the tablet has a different host entry for each address.
# SUDOKU_SSH_KEY changes the key file (default: ~/.ssh/id_ed25519_remarkable).
set -eu

if [ $# -lt 2 ]; then
	echo "Usage: $0 HOST 'COMMAND'" >&2
	exit 2
fi

host=$1
shift

exec ssh -o ConnectTimeout=5 -o BatchMode=yes -o IdentitiesOnly=yes \
	-o StrictHostKeyChecking=accept-new -o LogLevel=ERROR \
	-i "${SUDOKU_SSH_KEY:-$HOME/.ssh/id_ed25519_remarkable}" "root@$host" "$@"
