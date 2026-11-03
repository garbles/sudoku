---
name: remarkable-wifi
description: Turn SSH over Wi-Fi on or off on the reMarkable Paper Pro (rm-ssh-over-wlan). Use when the user asks to enable, disable or examine SSH over Wi-Fi (WLAN) on the tablet. Argument - on, off or status.
---

# Turn SSH over Wi-Fi on or off

Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

The tablet has the program `/usr/bin/rm-ssh-over-wlan`. `on` makes the marker file
`/data/internal/rm_enable_ssh_wifi_marker` and starts `dropbear-wlan.socket`. `off` deletes the
file and stops the socket. The setting stays after a restart.

This skill does not connect the tablet to a Wi-Fi network. The user does that on the tablet
(**Settings > Wi-Fi**).

## SSH command

Use `scripts/tablet-ssh.sh HOST 'COMMAND'` for all SSH commands (refer to the `remarkable-host`
skill). Do not put the SSH options in a shell variable: zsh does not split it.

## Procedure

1. **Get the action.** The argument is `on`, `off` or `status`. If there is no argument, ask the
   user (AskUserQuestion).

2. **Select the address.**

   - `on`: use the USB connection `10.11.99.1`. SSH over Wi-Fi is off, thus the Wi-Fi address does
     not accept a connection.
   - `off` and `status`: use `10.11.99.1` first. If it fails, use `REMARKABLE_HOST` from `.env`.

   If no address accepts a connection, tell the user to connect the USB cable and to make sure that
   the tablet is on. Then try again. Do not try more than two times without a new answer from the
   user.

3. **Caution for `off` on Wi-Fi.** If the connection is on Wi-Fi, tell the user that the
   connection stops after the command. After this, only the USB connection works. Ask the user to
   confirm before you continue.

4. **Run the command.**

   - `on` or `off`: `rm-ssh-over-wlan on` or `rm-ssh-over-wlan off`.
   - `status`: `[ -e /data/internal/rm_enable_ssh_wifi_marker ] && echo on || echo off; systemctl is-active dropbear-wlan.socket`.

5. **Examine the result.**

   - After `on`: run `true` on the Wi-Fi address. Get the address from
     `ip -4 -o addr show wlan0` on the USB connection. If `REMARKABLE_HOST` in `.env` is empty or
     different, tell the user and offer the `remarkable-host` skill.
   - After `off`: run `systemctl is-active dropbear-wlan.socket` on the USB connection (if
     connected). The result must not be `active`.

6. **Report** the new state and the Wi-Fi address (if there is one).
