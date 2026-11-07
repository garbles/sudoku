---
name: remarkable-debug
description: Examine the Sudoku app on the reMarkable Paper Pro when the screen output is wrong (gaps in the ink, ghosts, flashes, slow or missing updates) or when you change present.rs, the qtfb client or the ink patch. It gives the test settings of the app, the screen update log of xochitl, the lessons about screen updates and the debugging techniques. Use when the user reports a problem on the tablet screen, or asks to debug, examine or log the screen updates.
---

# Debug the app on the tablet

Write all messages and files in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

This skill tells how to examine the app on the reMarkable Paper Pro. DECISIONS.md records the
results. The commands use `scripts/tablet-ssh.sh HOST` (refer to the `remarkable-host` skill).

Do not put xochitl internals (addresses, hashes, decompiled names) in the files of this skill.

## Procedure

1. Read "Lessons" below. Many problems on the screen have a cause that is given there.
2. Find the tablet (`remarkable-host` skill), and make sure that the dev build is the current one
   (`remarkable-install-dev` skill, with a version name that tells the problem or the change).
3. Make the problem visible in a log:
   - The input and the settings of the app: [techniques/app-settings.md](techniques/app-settings.md).
   - What gets to the panel: [techniques/xochitl-update-log.md](techniques/xochitl-update-log.md).
     It needs a restart of xochitl. **Ask the owner before each restart.**
   - The pen area of the ink patch: [techniques/pen-area.md](techniques/pen-area.md).
   - The timing of a stroke from the journal: [techniques/stroke-timing.md](techniques/stroke-timing.md).
4. Ask the user to do the test on the tablet. Then read the log:
   [techniques/read-the-update-log.md](techniques/read-the-update-log.md).
5. Make a test on the computer that shows the same problem, and make sure that it fails before the
   fix: [techniques/reproduce-in-a-test.md](techniques/reproduce-in-a-test.md).
6. Change one thing at a time. Install the build, and ask the user to do the test again. Record each
   result in DECISIONS.md.
7. Add new lessons to this file, and new techniques to `techniques/`.

## Lessons

- **Qt merges the updates of one frame.** AppLoad gives each qtfb update to Qt as a dirty rectangle.
  All the updates that come before the next frame become one update: their bounding box. The log
  shows one update for each ink tick (approximately 30 ms apart), never two. Thus the app must send
  at most one rectangle for each tick. If it sends more, the panel gets their bounding box.
- **An update can stop an older update.** When a new update overlaps an update that is still in
  progress, the overlapping part of the older update stops. A pixel that the older update changed,
  but did not complete, can stay gray or white. This made gaps in the ink. An update that does not
  change pixels of the older update in progress does not cause gaps. Thus the app records the
  changed 4x4 px cells of each update and the time when they are complete (`present::Scheduler`).
- **Alignment.** The panel aligns each update to 4 px (x) and 2 px (y), and AppLoad adds 1 px at the
  right and at the bottom. Two rectangles that do not overlap in pixels can overlap on the panel.
- **The refresh mode sets the waveform of the full window.** qtfb refresh mode 0 (`ufast`) uses the
  fast pen waveform of xochitl. With it, the ink had no gaps, but the colors did not refresh fully
  and a faded image of the old screen stayed. The UI mode is necessary for the colors.
- **A change of the refresh mode is slow.** AppLoad refreshes the full window (a flash), and its
  server waits 1 s after each `SET_REFRESH_MODE` message. Do not change the mode while the app runs.
- **A screen mode can be set for an area.** A screen mode item of xochitl in the AppLoad window
  sets the mode for its area only. A Pen area over the cell of the ink gives ink with no gaps, and
  the UI mode stays for the rest. When the area starts and when it stops, xochitl refreshes the full
  area one time. Thus the area must not touch the cells around the written cell.
- **The Pen waveform cannot drive colors.** Gray and colored pixels that the Pen waveform changes get
  ghosts. Thus a redraw of the cell (the recognized digit) must wait until the Pen area stopped.
- **The pen area starts at each pen down on an empty cell, and it stays after the lift.** The
  area starts 2 to 12 ms after the pen down (measured with the area at the pen down), and the first
  ink waits until the patch applied it (45 ms). xochitl refreshes the full area when it starts, when
  it stops and when it changes (also when no pixel in it changed), and the ink patch cannot avoid
  this. Thus the area changes as rarely as possible: it follows the pen (`App::pen_cell`), it stays
  on its cell when the pen lifts, and it stops only before a redraw that needs the UI waveform in its
  cell (`FrameRecord::cell_change_shows_with_pen`: a number, a "?", other lines, a panel over the
  cell), when its cell becomes filled, or at a turn. The mark or the x of a tap goes at the pen lift
  inside the area, with the pen waveform, in the micro-cell only. An earlier version stopped the area
  at the lift of a tap: the mark waited for the stop (approximately 60 ms), and the stop refreshed
  the full cell with the ink dot of the tap on it. A move of the area gives two updates in the update
  log: the old cell (`ui`) and the new cell (`pen (ui)`). If gaps at the start of a stroke come back, the cause is
  probably the area that starts over ink while its update is in progress.
- **A tap shows ink, but not on a filled cell.** A tap on an empty cell shows a dot until its mark
  or its x removes it at the pen lift. A test build that hid the ink of a
  tap delayed the start of each stroke, and the owner selected the ink from the pen down. A pen
  touch that starts on a filled cell shows no ink and starts no area. The input log (`log-input`)
  shows if a touch was a tap: a pen down, few moves and a pen up in one micro-cell.
- **The redraw must wait for a pause in the writing, not for the recognition pause.** The move of
  the pen to the next cell can take more than a short pause. Thus the redraw waits until no stroke
  occurred for the idle time (`App::is_writing`). The recognition pause is 1 s
  (`RECOGNITION_PAUSE_MS`), and the idle time is equal to it. The digit is complete on the panel
  approximately 0.45 s after the read.
- **One source for a value.** An environment value in `external.manifest.json` overrode the source
  constant of the pause, thus a change of the source had no effect, and a wrong time was reported.
  Prefer one source. Compare the log with the expected time before you report a result.
- **AppLoad keeps the manifest in memory until xochitl restarts.** A change of the manifest on the
  tablet does not apply before that.
- **A running Sudoku keeps the old program after an install.** Compare the start time of the process
  with the time of the binary before you trust a test.
- **The tablet sends a touch from the hand after each pen lift.** It comes approximately 97 ms
  after the pen lift and ends a few ms before the next pen down. The `log-input` setting shows it.
  The app ignores a touch that starts less than 150 ms after a pen lift (`HAND_TOUCH_MS`).
- **The pen area can be off.** It is off when the patch file is newer than the start of xochitl.
  The log has the line `sudoku: pen region off`, and the ink is slower. Examine this first for an
  ink lag ([techniques/pen-area.md](techniques/pen-area.md)). `xovi/start` turns it on.
- **A restart of xochitl can fail.** One time in five restarts, `xovi/start` gave a segmentation
  fault of xochitl and a reboot of the tablet. The cause is not known. After the reboot, xovi
  operated. Tell the owner about this before a restart.
- **A redraw while the player writes stops the ink in the next cell.** The redraw of the digit in
  the first cell can be wide (it also changes pencil marks, highlights and completed groups), and it
  covers the next cell. Ink that waits for it does not show while the pen moves. Thus a redraw waits until the app
  has read the last ink, and ink in the pen area does not wait for older updates.
- **The layout of the drawing is a value of each thread.** The rotation and the hand of the player
  are thread-local values (`layout::rotation`, `layout::right_handed`). A drawing on a new thread
  starts with the portrait layout of a left-handed player. Each thread that draws must get the
  values of the main thread. A landscape screen that shows the board at the portrait position (and
  only a part of the board in the board area) had this cause in `render::draw_board_stripes`. The
  landscape screens are examined on the computer (refer to
  [techniques/check-the-orientations.md](techniques/check-the-orientations.md)).
- **One process drives the panel.** AppLoad runs in xochitl, thus the app must use the framebuffer of
  xochitl through qtfb. The app cannot drive the panel itself.
- **There is no screen layer for the pen.** xochitl has one framebuffer. Its "layers" are layers of
  the document or render caches, not planes of the panel. The only setting for an area is its
  screen mode. Temporary ink and the image below it share the same pixels.
- **After an update of the tablet software, xovi makes xochitl crash.** The hashtab of
  qt-resource-rebuilder is valid only for one software version ("only valid for QML environment
  version ... Loading skipped"). Then AppLoad cannot resolve its hooks ("Couldn't resolve the hashed
  identifier ... required by AppLoad hooks"), xochitl stops with a core dump three times, and the
  tablet restarts. Examine `journalctl -b -1` for these lines. Fix: run
  `/home/root/xovi/rebuild_hashtable` (detached, with an empty line on its input; the owner enters
  the passcode on the tablet during the run), then `/home/root/xovi/start`. A restart of the
  tablet can install a downloaded update.

## Notes for agents

- A new git worktree does not contain `scripts/fonts/rmsans-*.ttf` (git ignores them). Copy them
  from the main checkout, or the render tests and the Docker build fail.
- The memory of `bd` gives the path of `docker` (OrbStack) when `docker` is not on `PATH`.
- A test of the screen output needs the tablet: the tests on the computer cannot show the behavior
  of the panel. Change one setting at a time, and record each result in DECISIONS.md.
- The ink patch (`appload/sudoku/sudoku-penregion.qmd`) uses names of xochitl and AppLoad. After an
  update of the tablet software or of AppLoad, make sure that it still operates
  ([techniques/pen-area.md](techniques/pen-area.md)).
