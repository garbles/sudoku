---
name: remarkable-install-dev
description: Build the current working tree for the reMarkable Paper Pro and install this development build on the tablet over Wi-Fi or USB. Use when the user asks to install, deploy, copy or try the dev build, or local changes, on the tablet.
---

# Install the development build on the tablet

Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

## SSH command

Use the SSH command of the `remarkable-host` skill (installer key, `accept-new`). In the steps
below, `HOST` is the address that you select in step 1.

## Procedure

1. **Find the tablet. Wi-Fi first.**

   1. Read `REMARKABLE_HOST` from `.env`. If it is not empty, run `true` on it. If it succeeds,
      `HOST` is this address.
   2. If it is empty or fails, use the `remarkable-host` skill to find the Wi-Fi address. Do not
      ask its questions yet: only examine `.env` and the USB connection.
   3. If there is no Wi-Fi connection, run `true` on `10.11.99.1`. If it succeeds, `HOST` is
      `10.11.99.1`. Tell the user that the install uses USB, and offer the `remarkable-host` skill
      after the install.
   4. If there is no connection, ask the user (AskUserQuestion) to connect the tablet with the USB
      cable and to make sure that it is on. Then do step 1.3 again. Do not try more than two times
      without a new answer from the user.

2. **Examine AppLoad and xovi on the tablet.**

   - `[ -f /home/root/xovi/extensions.d/appload.so ]` must succeed. If not, AppLoad is not
     installed: tell the user to run the installer (README.md, "Installation"), and stop.
   - `p=$(pidof xochitl) && grep -q xovi.so /proc/${p%% *}/maps` tells if xovi runs. If not, tell
     the user that AppLoad is not in the menu until xovi starts. Offer to run
     `/home/root/xovi/start` after the install.

3. **Build.** Run the Linux Docker build. It runs all tests on aarch64 and writes `dist/sudoku/`:

   ```bash
   docker build --platform linux/arm64 --output type=local,dest=dist .
   ```

   If `docker` is not on `PATH`, use OrbStack:
   `DOCKER_HOST=unix://$HOME/.orbstack/run/docker.sock /Applications/OrbStack.app/Contents/MacOS/xbin/docker`.
   The build is slow (minutes): use a timeout of 600000 ms or run it in the background. If the
   build fails, show the error and stop. Do not install an old `dist/`.

   The build needs `scripts/fonts/rmsans-400.ttf` and `scripts/fonts/rmsans-500.ttf`. If they are
   not there, refer the user to `scripts/fonts/README.md`, and stop.

4. **Install.** Send `dist/sudoku` as a tar stream and replace the old folder in one step, as
   `install.sh` does. This is safe also when the app runs. `COPYFILE_DISABLE=1` stops macOS `tar`
   from adding `._*` files:

   ```bash
   COPYFILE_DISABLE=1 tar -C dist -czf - sudoku | ssh ... root@HOST 'set -e; d=/home/root/xovi/exthome/appload; t=$d/.sudoku-install; rm -rf "$t"; mkdir -p "$t"; tar -xzf - -C "$t"; chmod 755 "$t/sudoku/sudoku"; rm -rf "$d/sudoku.old"; if [ -d "$d/sudoku" ]; then mv "$d/sudoku" "$d/sudoku.old"; fi; mv "$t/sudoku" "$d/sudoku"; rm -rf "$d/sudoku.old" "$t"'
   ```

5. **Examine the result.** Compare `sha256sum dist/sudoku/sudoku` (on the computer: `shasum -a 256`)
   with `sha256sum /home/root/xovi/exthome/appload/sudoku/sudoku` on the tablet. They must be
   equal.

6. **Report.** Give the address (Wi-Fi or USB), the commit (`git rev-parse --short HEAD`, and
   "with local changes" if `git status --porcelain` is not empty) and the checksum. Tell the user:
   if Sudoku is open, close it and open it again from AppLoad. The open app continues with the old
   program.

Do not commit or push. This skill changes only `dist/` and the tablet.
