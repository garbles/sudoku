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
   (`remarkable-install-dev` skill).
3. Make the problem visible in a log:
   - The input and the settings of the app: [techniques/app-settings.md](techniques/app-settings.md).
   - What gets to the panel: [techniques/xochitl-update-log.md](techniques/xochitl-update-log.md).
     It needs a restart of xochitl. **Ask the owner before each restart.**
   - The pen area of the ink patch: [techniques/pen-area.md](techniques/pen-area.md).
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
- **The pen area and the tap.** The app knows that a touch is writing only when the pen leaves the
  micro-cell. If the area starts at the pen down, each tap starts it, and the mark waits until the
  area stops (approximately 60 ms, then a refresh of the full cell). Thus the area starts when the
  touch becomes writing (`App::writing_stroke`). The ink in the micro-cell before that goes with
  the UI waveform. If gaps at the start of a stroke come back, the cause is probably the area that
  starts over this ink while its update is in progress.
- **A tap shows ink.** A tap that changes nothing (a mark that a peer hides) shows a dot until a
  redraw removes it. A test build that hid the ink of a tap delayed the start of each stroke, and
  the owner selected the ink from the pen down. The input log (`log-input`) shows if a touch was a
  tap: a pen down, few moves and a pen up in one micro-cell.
- **The redraw must wait for a pause in the writing, not for the recognition pause.** The move of
  the pen to the next cell can take more than the recognition pause (0.5 s). Thus the redraw waits
  until no stroke occurred for 1 s (`App::is_writing`).
- **A redraw while the player writes stops the ink in the next cell.** The redraw of the digit in
  the first cell can be wide (it also changes pencil marks and highlights), and it covers the next
  cell. Ink that waits for it does not show while the pen moves. Thus a redraw waits until the app
  has read the last ink, and ink in the pen area does not wait for older updates.
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
