---
name: remarkable-host
description: Find the Wi-Fi IP address of the reMarkable Paper Pro and write it to REMARKABLE_HOST in .env. Use when the user asks to find the tablet, set or fix REMARKABLE_HOST, or when another skill cannot connect to the tablet on Wi-Fi.
---

# Find the tablet and set REMARKABLE_HOST

Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

## SSH command

Use the SSH key of the installer (`install.sh`). Use these options for all SSH commands to the
tablet:

```bash
ssh -o ConnectTimeout=5 -o BatchMode=yes -o IdentitiesOnly=yes \
    -o StrictHostKeyChecking=accept-new -o LogLevel=ERROR \
    -i "${SUDOKU_SSH_KEY:-$HOME/.ssh/id_ed25519_remarkable}" root@HOST 'COMMAND'
```

`accept-new` is necessary: the tablet has a different host entry for each address. If the key
file is not there, tell the user to run the installer one time (README.md, "Installation"). It
makes the key and adds it to the tablet. Do not ask for the SSH password of the tablet.

## Procedure

1. **Examine the current value.** Read `REMARKABLE_HOST` from `.env` (if the file is there). If the
   value is not empty, run `true` on that address. If the command succeeds, tell the user that the
   value is correct, and stop.

2. **Find the address on the USB connection.** Run this command on `10.11.99.1`:

   ```bash
   ip -4 -o addr show wlan0 | sed -n 's/.* inet \([0-9.]*\)\/.*/\1/p'
   ```

   - If the command succeeds and gives an address, go to step 3.
   - If the command succeeds but gives no address, the tablet is not on a Wi-Fi network. Tell the
     user to connect the tablet to Wi-Fi (**Settings > Wi-Fi** on the tablet), then do step 2 again.
   - If the SSH connection fails, go to step 4.

3. **Make sure that SSH over Wi-Fi works.** Run `true` on the Wi-Fi address.

   - If the command succeeds, go to step 5.
   - If the command fails, SSH over Wi-Fi is off. Ask the user (AskUserQuestion) if you can turn it
     on for them. If yes, use the `remarkable-wifi` skill (`on`), then do step 3 again. If no, stop
     and tell the user that `.env` did not change.

4. **No connection.** Ask the user (AskUserQuestion). Give these causes in the question:

   - The tablet is off or in sleep mode. Tell the user to push the power button.
   - The tablet is not on Wi-Fi, or SSH over Wi-Fi is off.
   - The USB cable is not connected, or developer mode is off.

   Give these options:

   - **"I connected the USB cable"**: do step 2 again.
   - **"Turn on SSH over Wi-Fi for me"**: tell the user to connect the USB cable (this is
     necessary to turn it on), then use the `remarkable-wifi` skill (`on`), then do step 2 again.
   - **"I know the address"**: use the address that the user gives, and go to step 3.

   Do not try the same connection again more than two times without a new answer from the user.

5. **Write `.env`.** If `.env` is not there, copy `.env.sample` to `.env`. Set the line
   `REMARKABLE_HOST=<address>`. Keep all other lines. Do not commit `.env` (git ignores it).

6. **Report.** Give the address and tell the user that the router can give the tablet a different
   address after a restart. A DHCP reservation on the router prevents this.
