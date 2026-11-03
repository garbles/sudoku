# Examine the pen area of the ink patch

The ink patch (`appload/sudoku/sudoku-penregion.qmd`) puts an area with the Pen screen mode of
xochitl in the AppLoad window (REQUIREMENTS.md §14.4). The app writes the area to
`/tmp/sudoku-pen-region` ("x y width height", or empty), and the patch reads it each 15 ms.

## Does the patch operate?

1. The patch is installed, and xochitl started after it. xochitl reads the patch only when it
   starts. Compare the time of the file with the start of xochitl (the tablet has BusyBox: `ls` has
   no `--time-style`, but `stat -c` operates):

   ```bash
   scripts/tablet-ssh.sh HOST 'stat -c %y /home/root/xovi/exthome/qt-resource-rebuilder/sudoku-penregion.qmd; systemctl show xochitl -p ActiveEnterTimestamp'
   ```

2. If the patch is newer, run `/home/root/xovi/start` (ask the owner first).

3. The app uses the area. When it starts, the app writes `sudoku: pen region on` to the log of
   xochitl. `off` means that the patch file is not there, or that it is newer than the start of
   xochitl.

4. The area gets to the panel. In the update log
   ([xochitl-update-log.md](xochitl-update-log.md)), the ink updates show as `pen (ui)`. If they
   show as `ui` while `/tmp/sudoku-pen-region` is not empty, the patch does not apply the area: for
   example, an update of the tablet software or of AppLoad changed the names that the patch uses.

## Watch the area while the user writes

```bash
scripts/tablet-ssh.sh HOST 'for i in $(seq 200); do cat /tmp/sudoku-pen-region; echo; sleep 0.05; done' | uniq
```

Each line is one area. While the player writes in a cell, the area is the inside of that cell. When
the player stops, the file is empty.
