# Examine the pen area

The app gives AppLoad an area with the Pen screen mode of xochitl (REQUIREMENTS.md §14.4). The
values are in the coordinates of the window of AppLoad (the screen as the player sees it), not in
framebuffer coordinates: in landscape, `x` is in 0..2160 and `y` is in 0..1620. There are two modes.

- **Frontend mode** (AppLoad 0.6.0 or newer). The QML frontend (`appload/sudoku/ui/main.qml`) shows
  the framebuffer in an AppLoad `DisplayMethodArea`, and a second `DisplayMethodArea` has the Pen
  mode (value 0) over the pen area. The program sends the area to the frontend as a message
  (`launch::MSG_PEN_AREA`, "x y w h" or `none`), and the frontend converts it to its item.
- **External mode** (the older mode, and the colors app). AppLoad starts `sudoku` as an external
  program. The older ink patch (`sudoku-penregion.qmd`, now not in releases) reads the file
  `/tmp/sudoku-pen-region` each 15 ms. The file does not exist in the frontend mode.

## Which mode runs?

AppLoad reads the manifests only when the interface starts and at "Reload" in the AppLoad menu. After
a change of the manifest type, a test can run the other mode. Always examine the log first.

```bash
scripts/tablet-ssh.sh HOST 'journalctl -u xochitl --since "-5min" --no-pager | grep -E "Starting (binary backend/entry|external binary)|sudoku: (launch|a frontend connected|pen region|rotation)"'
```

- "Starting binary backend/entry" (AppLoad) and `sudoku: launch Frontend` and
  `sudoku: a frontend connected` (program): the frontend mode.
- "Starting external binary" (AppLoad) and `sudoku: launch External(..)`: the external mode. Then the
  manifest is old or the type is `external`: tap "Reload" in the AppLoad menu (or run
  `/home/root/xovi/start`, ask the owner first), and test again.
- `sudoku: rotation` shows the rotation that the program got. If the frontend cannot read the four
  internal names of the AppLoad window (`globalRotation`, `swipeFromTheTop()`, `fullscreen`,
  `minimized`), the app stays in portrait and the log has one line about it: examine the names
  after each AppLoad update.

## Does the pen area operate in the frontend mode?

1. The log has `sudoku: a frontend connected`. If not, the frontend did not start (the program stops
   after 10 s without a frontend).
2. In the update log ([xochitl-update-log.md](xochitl-update-log.md)), the ink updates show as
   `pen (ui)`. If they show as `ui`, the pen area does not reach the panel: examine the frontend
   (the second `DisplayMethodArea`: the mode values are 0 pen, 3 UI, 4 content, and the names of
   3 and 4 are in the opposite order) and the log line `cannot send the pen area to the frontend`.

## Does the ink patch operate in the external mode?

1. The patch file is on the tablet, and xochitl started after it (xochitl reads the patch only when
   it starts). The tablet has BusyBox: `stat -c` operates, `ls` has no `--time-style`:

   ```bash
   scripts/tablet-ssh.sh HOST 'stat -c %y /home/root/xovi/exthome/qt-resource-rebuilder/sudoku-penregion.qmd; systemctl show xochitl -p ActiveEnterTimestamp'
   ```

2. The log line `sudoku: pen region on` means that the app writes the file. `off` means that the
   patch file is not there, or that it is newer than the start of xochitl.
3. If the ink updates show as `ui` while `/tmp/sudoku-pen-region` is not empty, the patch does not
   apply the area (for example, an update of the tablet software or of AppLoad changed the names).

## Watch the area while the user writes (external mode)

```bash
scripts/tablet-ssh.sh HOST 'for i in $(seq 200); do cat /tmp/sudoku-pen-region; echo; sleep 0.05; done' | uniq
```

Each line is one area. While the player writes in a cell, the area is the inside of that cell. When
the player stops, the file is empty. In the frontend mode, use the update log (`pen (ui)`).
