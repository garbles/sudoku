# Sudoku for reMarkable Paper Pro — Verification record

This document is written in ASD-STE100 Simplified Technical English (STE).

The owner confirmed the implementation decisions on 2026-10-25. They are now in `REQUIREMENTS.md`
(§14.5, §16) and `INVARIANTS.md` (INP-11 to INP-14, QST-4, CLK-6, NAV-12, ANA-11, PER-7). This
document records how the app was examined.

## Pixels

The app draws with tiny-skia. The images in `images/` come from ImageMagick (FreeType). The test
`tests/render.rs` compares the app with each image. Each pixel that differs is at the edge of a line or
a glyph (antialiasing): all differences are within 2 px of an edge, and all except a maximum of 2
pixels are within 1 px. A maximum of 0.6 % of the pixels differ by more than 25 %. The manual pages are
the images of `images/` in the app.

In `color-menu.png`, the redo button is black. UND-7 tells that redo is black only if a redo is
available. Thus the test makes the image state with one change that was undone.

## Handwriting

`src/recognizer.rs` has the test `writer_independent_accuracy`: the examples of writers 1 to 40 read the
handwriting of writers 41 to 60. Result: 96.7 % of the digits correct, 0.8 % a different digit, 90.3 %
of the other characters rejected.

## On the device (reMarkable Paper Pro, OS build 20261009073202), 2026-10-25

`sudoku --self-test OUT.png SECONDS` examines the device without AppLoad. Results:

| Item | Result |
|---|---|
| Font `/usr/share/remarkable/webui/reMarkableSans.woff2` | Loads in 19 ms. The screen drawn with it agrees with `images/screen-in-progress.png` (0.135 % of pixels differ, all at edges). |
| Drawing one screen | 65 ms |
| Recognizer | Ready in 1.7 s (in a thread when the app starts). One reading: 14 ms. |
| Eraser | The digitizer is `/dev/input/event2` ("Elan marker input"). The app reads `BTN_TOOL_PEN` and `BTN_TOOL_RUBBER` while xochitl operates. |

### Start from AppLoad, 2026-10-25

Vellum, `vellum add appload` (AppLoad 0.6.0, xovi 0.3.3) on OS 3.29.0.153. Results:

| Item | Result |
|---|---|
| Start | AppLoad starts `./sudoku` with qtfb. The screen shows the board, the meter and the menu correctly (screenshot with `scripts/rmpp-screenshot.sh`). |
| Pen taps | Taps added the marks 1, 3 and 5 at their keypad positions in the cell that the player tapped. |
| Writing | The handwritten 5 and 8 of the owner were read correctly. The 5 hid the mark 5 of a peer (MRK-4). |
| Eraser | Each rub with the eraser end made one `Erase` change. |
| Finger | The menu button and the close button operate with a finger. |
| Close and open again | The close button saved the game and stopped the app (exit code 0). The app opened again with the same seed, and the clock continued from the saved time (122.974 s; 27.6 s later 150.551 s, for 28 s open). |
| SIGTERM | The app saves and stops (exit code 0). |

### States of the menu and the panels, 2026-10-25

Each check compares a screenshot of the device with the saved game (`game.json`, `settings.json`).

| State | Saved data | Screen |
|---|---|---|
| New game, menu open | undo 0, redo 0 | Undo and redo gray (gray70) — UND-7 |
| After one pencil mark | undo 1, redo 0 | Undo black, redo gray |
| After undo | undo 0, redo 1 | Undo gray, redo black |
| Cell colors panel, Yellow for starting numbers | `color_starting: Yellow` | Black box on Yellow; Yellow crossed out in the other two sections; Red and Green crossed out for starting numbers; starting-number cells yellow; paint bucket white on black (COL-2, COL-5, BRD-7, NAV-8) |

The app also drew the same saved state on the computer (`examples/render_state.rs`). Inside the outer
4 px, the screenshot and that drawing agree (mean difference 0.03; no pixel differs by more than 25 %).
At the outer 2 px, AppLoad draws a black line around each window (`window.qml`, `border.width: 2`).
The app cannot remove it.

### Owner examination, 2026-10-25

| Item | Result |
|---|---|
| E-ink (R§16): faint micro-cell lines, pale colors, gray icons | All clear. |
| Recognition pause of 0.5 s (R§16) | Numbers with two strokes stay together; single digits show quickly. |
| Handwriting of the owner | 5, 8, 1, 2, 3, 6 and 7 read correctly. Six 4s: five read as 4, one as 9. |

### Ink while the pen writes, 2026-10-25

The owner asked to see the strokes while writing. Findings on the device and the changes:

| Finding | Change |
|---|---|
| A full drawing takes approximately 65 ms, and the app drew the full screen for each pen movement. The ink came 0.3 to 1 s late. | `App::redraw` and `App::view_key`: a full drawing only when the screen changes. New points of the stroke are drawn into the frame, and only their area is sent. The property test `no_redraw_means_no_change_on_the_screen` examines this. |
| AppLoad repaints for each update. One update for each pen event made a long queue. | At most one ink update each 30 ms (`INK_UPDATE_INTERVAL`). |
| A change of the refresh mode makes AppLoad refresh the full window (a flash). | One refresh mode (UI) for the full time. |
| The panel skips pixels of an update that overlaps an update in progress. The ink had gaps until the next update of the area. A screenshot showed solid ink in the image of xochitl; a test with a grid of black dots showed that gray pixels are not the cause. | 400 ms after the last ink update, the app sends the full ink area again (`INK_REPAIR_DELAY`). The gaps fill in. Replaced on 2026-11-01 (next row). |

### Ink gaps while the pen writes, 2026-11-01

The `remarkable-debug` skill gives the procedures (test settings, the screen update log of xochitl)
and the lessons.

| Finding | Change |
|---|---|
| Device test with refresh mode 0 (`ufast`): no gaps in the ink, but the colors do not refresh fully and a faded image of the old puzzle stays after a full change. Thus the gaps come from overlapping updates with the slow UI waveform. | Refresh mode UI stays. The setting `refresh-mode` selects another mode for a test. |
| First try: the app sent only the parts of new ink outside the updates in progress (up to 4 rectangles in one tick). The gaps stayed. The update log of xochitl showed one update for each tick, with the bounding box of the parts. | — |
| Second try: one rectangle for each tick. The gaps are fewer but not gone. The log shows new ink over older ink 400 to 560 ms after the older update: the UI waveform can take more than 400 ms. | `present::Scheduler`: one rectangle for each ink tick (4x4 px cells). It never touches a cell that an update in progress changes (`INK_WAVE_MS`; the setting `ink-wave-ms` changes it for a test). While the pen moves, the cells at the pen tip wait one tick, because the next segment starts there. A change of the screen does not wait; the ink that it stops is sent again later. The 400 ms repair is removed. |
| Device test with `ink-wave-ms` = 1000: the holes fill in, but slowly. Content mode: worse than UI. | — |
| Prototype: a QML patch of AppLoad (now `appload/sudoku/sudoku-penregion.qmd`) puts a Pen screen mode area over the board. Ink: no gaps. Colored cells in the area: ghosts. | — |
| Pen area only on the cell of the ink, while the player writes. The app gives the area to the patch in `/tmp/sudoku-pen-region`; the patch reads it each 15 ms. Ink waits 45 ms after the area starts; the redraw waits 45 ms after it stops. Device test: ink looks great, no ghosts, no flash. The cells around the written cell refreshed, because the area had a margin. | The area is the inside of the cell, aligned inward to the 4 px grid. Device test: pending. |
| Owner report: with fast writing in many cells, the full redraw sometimes started when the pen touched the next cell, and the new ink did not show until the redraw was complete. Cause: the redraw waited only until the app read the last ink (0.5 s after the last lift), and the movement to the next cell can take longer. | `App::is_writing`: one writing timer for all cells, 1 s after the last stroke on the board (owner decision 2026-11-02; first 1.5 s). A press that does not write stops it. Device test: pending. |
| Owner report: a tap on a mark that a peer hides left a dot of ink on the screen until a redraw. Cause: the app showed the ink from the pen down, before it knew that the touch is a tap. Input log: each tap is a pen down, one move and a pen up. | `App::live_ink`: no ink while the touch can be a tap (later removed: refer to option 2 below). When the pen leaves the micro-cell, all of the stroke shows. The pen area still starts at the pen down. Device test: pending. |
| Owner decision 2026-11-02: remove the micro-cell lines. The micro-cells keep their positions (keypad order) for taps and pencil marks. | `render::draw_cell`, `scripts/render.sh`, `scripts/manual-lib.sh`; BRD-6 changed. |
| Owner request 2026-11-02: a pencil mark must show much faster. Cause of the delay: a tap started the pen area at the pen down, and the mark waited until the area stopped (15 ms poll of the patch + 45 ms), then xochitl refreshed the full cell. | The area starts when the touch becomes writing (`present::wanted_region` uses `App::writing_stroke`). A tap sends only the changed pixels of the mark, at once. Risk: an area that starts over an update in progress (a stroke immediately after a mark in the same cell). Device test: pending. |
| Owner decision 2026-11-02 (option 2): the ink shows again from the pen down, also for a tap (the hidden ink delayed the start of each stroke). The pen area still starts only when the touch becomes writing. The micro-cell lines come back, as faint as possible: 2 px `gray93` (one gray level lighter than the old `gray80`, if the panel has 16 levels). | `App::writing_stroke`; `render::MICRO_LINE`. Device test: pending (are the lines visible; gaps at the start of a stroke). |
| Owner device test 2026-11-02: option 2 "feels great". The `gray93` lines are visible but too faint: a little darker. | `render::MICRO_LINE` = `gray90`, then `gray87` (owner: "darker again"). |
| Owner request 2026-11-02: a setting for the gray of the micro-cell lines in the Colors panel, with a range of grays and no lines. Owner selections: a 3×3 grid section (None + 8 grays: 95, 93, 90, 87, 83, 80, 75, 70); all four sections get cells of 112×90 px (sections of 360 px), so that the panel fits in landscape. | `settings::LineGray`, `Settings::micro_line` (default gray87), `layout::line_gray_cell`, `render::color_section`. Reference states `color-menu-lines-95.png` and `color-menu-lines-70.png`. |
| Owner change 2026-11-02: the grays go down to gray40: None, 90, 80, 70, 60, 55, 50, 45, 40. gray87 is not in the list, thus the default is gray90 (the nearest value). | `settings::LineGray`; reference states `color-menu-lines-90.png` and `color-menu-lines-40.png`. |

### Owner changes, 2026-10-25

| Change | Where |
|---|---|
| Landscape layout (REQUIREMENTS.md §3.3). AppLoad sends the rotation (`supportsRotation`); the app draws its screen into the portrait framebuffer with the rotation and maps the input back. On the device, AppLoad sends 2 for xochitl's "InvertedLandscape". | `layout.rs` (rotation, orientation-aware values), `platform/mod.rs`, `scripts/manual-*-landscape.sh` |
| Darker cell colors (for example Gray `#C8C8C8`, Red `#F0A3A3`, Green `#B5DF9E`). | `settings.rs`, `scripts/toolbar.sh`, `scripts/board-state.py`, `scripts/manual-lib.sh`; images made again |
| "Insane" is now "Hard". | `render.rs`, `scripts/ui-lib.sh`, `scripts/analytics.py`, `scripts/manual-new-game.sh`; images made again |

### Landscape on the device, 2026-10-25 (clean start of the installed build)

| Check | Result |
|---|---|
| Rotation at the start | AppLoad sends the rotation 37 µs after the connection. The app waits for it (maximum 300 ms) before the first drawing, so it does not show a portrait screen first. |
| Manual page 1, landscape | The screenshot (inside AppLoad's 2 px line) is the same as `manual-how-to-play-landscape.png` (mean difference 0.00). |
| Analytics summary, landscape | The screenshot is the same as the app's drawing of the device's results database (mean difference 0.01). The games list (one table across the full width, 6 games on each page) is examined against `analytics-games-landscape.png` (`tests/render.rs`). |
| Games list, landscape, with results | `sudoku --test-results add` added 14 test rows in the database (one transaction, while the app was closed). The screenshot of page 1 (two columns of 6, with 4 real results of the owner) is the same as the app's drawing of the device's database (mean difference 0.02). `--test-results remove` then deleted only the 14 test rows; the 4 real results stay and the integrity check is ok. |

### Recognizer, second version, 2026-10-25

The owner asked to improve the reading of 4/9 and of 8 with loops. Each digit example now also has
copies with a rotation of ±8° and a width of 0.8 and 1.2 (`DIGIT_COPIES`), and ALPHA is 1.4. For the
writers 41 to 60 (not in the examples of the test): 97.8 % of the digits correct (before: 96.7 %),
0.6 % a different digit (before: 0.8 %), 91.1 % of the other characters rejected (before: 90.3 %).
For each digit: 1, 2, 3 and 4 40 of 40; 5, 6 and 8 39 of 40; 7 38 of 40; 9 37 of 40.

### Rule-by-rule review, 2026-10-25

An independent review examined all 118 rules of INVARIANTS.md against the code and the tests.
Two violations were found and fixed, each with a test that failed before the fix
(`tests/rules.rs`):

- QST-1: an undo or a redo could put a number under the "?". Now a change that fills the cell of the
  "?" removes the "?".
- HLT-2, QST-4: a second finger press did not remove the highlight and the "?". Now it does.

Nine rules had weak tests; the review added a test for each. BRD-4 now includes undo of an erasure
(REQUIREMENTS.md §7 permits it). `read_ink` accepts only 1 to 9 from the recognizer (INP-6).

The device tool `sudoku --test-results add|remove` changes the results database outside completion
and Delete (ANA-2). It is only for an examination on a device; the app never uses it.

### Recognizer, third version: the owner's handwriting, 2026-10-25

With the ink log on the device (`log-ink`), the owner wrote five 4s, six 9s and six 8s. All 8s were
correct. The owner writes a closed 4 and a 9 with a straight tail, like the typeface, from the bottom
up; the database has almost no examples of these shapes in that direction. The recognizer now also has
typeface examples of 4 and 9, in both directions (`typeface_examples`). Result: all 17 samples of the
owner correct (`handwriting_of_the_owner_is_read`, not independent). For the writers 41 to 60: 97.8 %
correct, 0.8 % a different digit (one 7 read as 9), 91.1 % of the other characters rejected.

### A straight line is a 1, 2026-10-25

The owner asked that a straight vertical line, written down or up, is a 1. Two causes stopped it: the
characters `:`, `;` and `.` of the database (their dots are in a vertical row, so their pen path is a
vertical line) were "not 1–9" examples, and almost all 1s of the database are written down. The
examples now exclude `:`, `;` and `.` (assets/README.md), and the typeface examples include a
vertical line in both directions. Test: `a_straight_line_is_a_1_in_both_directions`. For the writers
41 to 60: 97.8 % correct, 0.8 % a different digit, 91.3 % of the other characters rejected.

### Review of the last changes, 2026-10-25

An independent review of the recognizer changes, the ink log and the `--test-results` tool found ten
items. Fixed: one typeface 4 example was a cross (removed); `--test-results` accepts only `add` or
`remove`; a second `add` does not make duplicates; the test rows have the starting numbers of their
seed and mode (PUZ-5, CMP-5); the reversed typeface examples are made in `typeface_examples`; the
file name of the database is in one place (`store::RESULTS_FILE`); examples with a class that is not
0 to 9 are not used. The owner then wrote 4s, 9s and straight-line 1s (down and up) on the device with
the final build: all were read correctly.

### Owner requests: touch bug, Settings panel, Statistics, 2026-10-25

The new text is in REQUIREMENTS.md (§9, §9.1, §6.1, §6.2, §13.1, §14.2, §15) and INVARIANTS.md
(INP-15, MRK-9, HLT-6, NAV-14, ANA-12 and changes to MRK-4, HLT-3, NAV-12, UND-2, CMP-6, ANA-2,
ANA-8, PER-3). A sub-agent examined the new text for contradictions; its 6 contradictions and 9
missing items are corrected.

- Touch bug (INP-15): the input log of the device showed a touch (id 1384) with a press and moves but
  no release. The app followed that touch and ignored all next finger presses as a second finger. Now
  the app forgets a touch when a new press has the same id or when that touch has had no event for
  0.15 s. Test: `a_touch_without_a_release_does_not_stop_the_next_touches` (the events of the log).
- Settings panel (gear, top slot 2; undo and redo are in slots 3 and 4): two switches, kept in
  settings.json. Image: `settings-menu.png`.
- Statistics: the new name in the app, the manual and the documents (the image files keep the name
  "analytics"). "Delete all games" with a confirmation. Images: `analytics-delete-all.png` and its
  landscape version.

The owner examined all three on the device: finger taps on the menu keep working, the two switches
operate, and "Delete all games" asks for a confirmation.

### Owner request: seven technique-based modes, 2026-10-25

The owner asked for two more difficulty levels. The owner decided:

- The two new modes are **more difficult** than the old Hard: modes 5 and 6. The modes 0 to 4 keep
  their numbers, thus a saved `last_mode` and the results keep their meaning.
- **All seven modes** come from the techniques that a person must use, not from the quantity of
  starting numbers: 0 singles with 44 starting numbers; 1 singles; 2 locked candidates; 3 subsets;
  4 X-Wing, Swordfish, XY-Wing; 5 Skyscraper, 2-String Kite, XYZ-Wing, W-Wing, simple colouring,
  Jellyfish; 6 chains (AIC). No mode needs a guess or a uniqueness technique.
- **Migrate** the old data. A game file with no generator version is version 1, and the app examines
  it with the old generator (PUZ-5). The results table goes to schema version 2 (`PRAGMA
  user_version`). Rows that break ANA-1 stay in the table `results_v1`.
- The landscape Statistics page has **shorter rows** (76 units) in the "By difficulty" table. Portrait
  does not change.
- The meters in the filter segments are **smaller** (18 units between the stops).
- The first start stays at **mode 2**.

The background research is in `research/puzzle_generation.md` (Simon Tatham's "Solo" method).

Examination:

- `solver::tests::steps_never_remove_the_solution`: on 300 puzzles of version 1 (made without the
  solver, thus also very difficult ones), no step of any technique removes the digit of the only
  solution.
- `solver::tests::each_mode_needs_its_level` and the benchmark `examples/gen_bench.rs`: 700 puzzles
  (100 seeds for each mode) need exactly the level of their mode. All techniques occur in the
  puzzles, except quads and Jellyfish, which are rare; hand-made boards examine them.
- `generator::tests::version_1_puzzles_do_not_change`: three puzzles of version 1 are the same as the
  puzzles of commit 0b855b0.
- Time on the development computer (Apple Silicon, release build): mean 0.002 s (mode 0) to 0.13 s
  (mode 4); maximum 0.64 s. The time on the tablet is not measured yet.
- The reference images were made again with `scripts/build.sh`. Changed: the images with a meter or a
  slider, the analytics images and the "New game" manual page ("Easy needs only simple logic. Hard
  needs advanced logic.").

### Owner request: the toolbar follows the handedness, 2026-10-26

The owner configured the device for a left-handed user. For a right-handed user, the menu must be at
the left edge and the close button at the right (REQUIREMENTS.md §3.1, NAV-15).

- xochitl keeps the handedness in `/home/root/.config/remarkable/xochitl.conf`, section `[General]`.
  On the device of the owner the value is `RightHanded=false` (examined through SSH). If the value is
  not there, the app uses right-handed, as the device does by default.
- The app reads the value only when it starts. A change in the settings of the device shows when the
  app starts again.
- The menu icon is a mirror image for a right-handed user (owner decision). The other icons do not
  change, for example the undo arrow still turns to the left.
- The reference images in `images/` stay left-handed. Only `color-menu-right-handed.png` shows the
  right-handed layout. `scripts/toolbar.sh` makes it from `color-menu.png`: the parts move by full
  pixels and only the menu icon is a mirror image.
- The demo videos use the right-handed layout, as the device does by default (`hand right` in
  `video/demo.txt` and `video/demo-full.txt`).

### Owner request: the menu is open by default, 2026-11-02

The owner asked: "make it so that the menu is open by default", and then: "the menu must stay open in
both of those cases" (after "< Back" and after "Start new game"). The owner examined the behavior on
the device (Paper Pro, dev build) and selected "Correct". It is now in REQUIREMENTS.md (§2, §3, §9,
§11, §12, §13.1, §14.3) and INVARIANTS.md (NAV-1, NAV-3, NAV-5, NAV-9, NAV-16, PER-2).

| Case | Examined result |
|---|---|
| The app opens (a new game or a saved game) | The game screen with the menu open: the toolbar and the close button show. No panel, no highlight and no "?". The app does not keep the menu state. |
| "< Back" from the manual | The game screen with the menu open and no panel open. |
| "< Back" from Statistics | The game screen with the menu open and no panel open. |
| "< Back" from the games list | No change: Statistics shows. |
| "Start new game" | The new puzzle shows. The New game panel closes and the menu stays open. |
| Other behavior | No change. The menu button closes and opens the menu, the board operates while the menu is open, the close button closes the app, and the right-handed layout is a mirror image. On the manual, statistics and games list screens the menu is closed. |

- No new positions, sizes, colors, fonts or text. The open menu is the same as in
  `screen-toolbar.png` (REQUIREMENTS.md §3.1, §9).
- `images/screen.png` stays as the image of the closed menu. It is not the start state now.
  `reference::app` closes the menu, and each reference state that shows the open menu opens it.
  Thus the images do not change.
- The event scripts of the demo videos (`video/demo.txt`, `video/demo-full.txt`) start with the menu
  closed (`Script::app`). The scripts show the empty board first and open the menu with a tap, and
  they have no "< Back" and no "Start new game". Thus the videos and the output hashes of the bench
  do not change.
- Rejected value: the first prototype opened the menu only when the app started, and "< Back" and
  "Start new game" closed it. The owner rejected this: the menu must stay open in these two cases
  also.

Review finding (HARD-1) and owner decision, 2026-11-02:

- In the left-handed layout, "< Back" (x 0–180, y 0–100) and the close button (x 0–112, y 0–112)
  are at the same position. "< Back" now shows the game with the menu open. Thus the second tap of a
  fast double tap on "< Back" closes the app. In the right-handed layout, the second tap hits the
  menu button and closes the menu.
- The owner selected "Ignore the 2nd tap": for 0.5 s after "< Back" shows the game screen, the app
  ignores a press on the toolbar (all buttons, also the menu button) and on the close button. A press
  on the board operates as usual. This is true for the two layouts and for "< Back" from the manual
  and from Statistics (REQUIREMENTS.md §9, INVARIANTS.md NAV-16).
- Rejected option: accept the risk and make no change.

### Owner request: the installer adds the power-button starter, 2026-11-03

The owner asked: "lets add triple-tap to the installer". The owner approved the terminal output of
`install.sh` in a simulation on 2026-11-03 (beads issue remarkable-sudoku-kuj). The tablet was not
reachable, thus the owner examined no restart of the tablet and no press of the power button on the
tablet. The open bead remarkable-sudoku-rjw records the device test. The change is now in
REQUIREMENTS.md (§14.4, §14.6). INVARIANTS.md does not change: its rules tell the states of the
app, and the installer runs on the computer. The rules INS-1 to INS-6 give the behavior.

| Item | Approved result |
|---|---|
| Steps | The installer has 11 steps. The new step 10 is "Power button starter". It is after step 9 "Start xovi" and before step 11 "Sudoku". The header comment of `install.sh` lists 11 steps. |
| Step 10, done already | The installer shows "The power button starts xovi. (done already)" and changes nothing. |
| Step 10, not done | The installer shows the two information lines, examines the internet connection of the tablet, shows "Installing the power-button starter...", and installs the starter with Vellum. Then it examines the service and shows "The power button starts xovi.". |
| The service is not on | The installer shows "Repairing the power-button starter...", runs the starter's `enable.sh` (else `vellum reenable` and `systemctl start xovi-tripletap`), and examines the service again. |
| A failure | The installer stops before Sudoku. Vellum: "Vellum did not install the power-button starter." The service: "The power-button starter is not on." Sudoku is not installed. |
| The final box | "After a restart, xovi is off. AppLoad is not in the menu." / "Press the power button 3 times to start xovi. The screen" / "starts again after some seconds." |

Owner decisions, 2026-11-03:

1. If Vellum cannot install the starter, the installer stops before Sudoku. The final box always
   tells the truth about the power button.
2. If the starter is on the tablet but the service is not on (for example after a tablet software
   update or a stopped service), the installer tries a repair, examines the service again, and
   stops if the service is still not on.
3. The manual installation in README.md and R§14.4 also installs the starter, and it tells the user
   about the power button.
4. README.md and the site warn the user: if xovi is on, a triple press starts the interface again
   and closes Sudoku.

Exact values, 2026-11-03:

- `STEPS=11`. The title of step 10 is "Power button starter". The skip text and the success text are
  "The power button starts xovi.".
- The two information lines: "After a restart, xovi is off, and AppLoad is not in the menu." and
  "A triple press of the power button starts xovi again.".
- The install command:
  `/home/root/.vellum/bin/vellum update && /home/root/.vellum/bin/vellum add tripletap`.
- The repair: `/home/root/xovi-tripletap/enable.sh` if it is present; else
  `/home/root/.vellum/bin/vellum reenable` and `systemctl start xovi-tripletap`.
- The condition for the skip and for the examination of the service: the systemd unit
  `xovi-tripletap` is enabled and active.
- The failure texts: "Vellum did not install the power-button starter." and "The power-button starter
  is not on.".
- No new environment variable. Steps 1 to 9 and step 11 do not change, except the numbers.
- No value was rejected.

Items that the owner did not examine in the simulation, 2026-11-03:

- The restart of the tablet and the physical power button: the simulation cannot prove them. The
  open bead remarkable-sudoku-rjw records the device test.
- The repair after an update of the tablet software. The owner examined it later on the device, on
  2026-11-03: it failed (refer to the bug record below). The repair is now automatic.
- Another starter that starts xovi at boot: the installer does not examine it. The box line "After a
  restart, xovi is off" is correct for a tablet with only this starter.
- An old version of the tripletap package (before 1.1.1): the installer does not examine the
  version. The Vellum package is the latest version.

### Bug: the installer stopped after an update of the tablet software, 2026-11-03

Beads bug remarkable-sudoku-3ed. The tablet software changed from 3.29.0.153 to 3.29.1.0. Then the
owner ran `install.sh` from the computer, and it stopped at step 10. Vellum printed:

- "warning: reenable has not been run since last OS upgrade. Run 'vellum reenable'..."
- "OS upgraded (3.29.0.153 -> 3.29.1.0). Run 'vellum upgrade'... Run 'vellum reenable'..."

The installer printed "Vellum did not install the power-button starter." The install command of the
starter failed, because Vellum blocks changes until the upgrade and the reenable. The systemd unit
`xovi-tripletap` was missing after the update, and the xovi hashtable was stale. The file
`/home/root/.vellum/state/osver` gave 3.29.0.153.

The owner recovered the tablet manually:

1. `/home/root/.vellum/bin/vellum upgrade` (syncs the OS version of Vellum).
2. `/home/root/.vellum/bin/vellum reenable` (restores qt-resource-rebuilder and the unit
   `xovi-tripletap`).
3. `/home/root/xovi/rebuild_hashtable`.
4. `/home/root/xovi/start`.

The tablet is recovered.

The fix is in `install.sh` and keeps 11 steps:

- Step 7 examines the Vellum state after an update of the tablet software: `vellum info remarkable-os`
  gives the warning "reenable has not been run". When Vellum says that the software changed, the step
  tells the user "The tablet software changed. The installer updates Vellum and the xovi files.",
  examines the internet connection of the tablet, shows the "Before you continue" box and pause,
  shows "Updating Vellum after the tablet software change...", runs `vellum upgrade` and
  `vellum reenable`, and then rebuilds the hashtable also when the hashtab file exists. The step
  remembers that xovi must start again.
- If the command fails, or Vellum still reports the stale state, the installer stops with "Vellum
  could not update the packages after the tablet software change." It does not rebuild the hashtable,
  does not start xovi and does not install Sudoku.
- Step 9 starts xovi again after the change, also when xovi is on. It shows "xovi must start again
  after the tablet software change." after the message for the ink patch when that also applies.
  The warning that the screen stops was already shown in step 7, thus step 9 has no second pause.
- Step 10 is unchanged: after the sync, it skips when the unit `xovi-tripletap` is enabled and
  active, and it installs the package when the package is absent.

The behavior is in REQUIREMENTS.md §14.6 (steps 7 and 9, INS-7 and INS-8). The automatic stale path
is examined in the simulation only (`tests/install/run.sh` with the fake ssh). The device was
recovered manually; the installer was not run on a stale tablet.

Remaining limitation (follow-up bead remarkable-sudoku-63a): if the user runs `vellum upgrade` and
`vellum reenable` manually, but not `/home/root/xovi/rebuild_hashtable`, Vellum does not report the
stale state. Then step 7 skips because the hashtab file exists, and the installer does not detect the
stale hashtable. The installer has no OS-version stamp for the hashtable.

### Bug: the landscape board was at the portrait position, 2026-11-03

Beads bug remarkable-sudoku-wg2. The owner reported: "when i turn the machine, the puzzle is no
longer re-rendered on the screen". The rest of the landscape screen (the toolbar, the meter, the
panels) was correct; the board moved to the position that it has in portrait, and the board area
showed only a part of it.

Cause: `render::draw_board_stripes` draws the board in stripes on threads (the first frame, and each
turn: a tile key contains the board origin, thus a turn makes the 85 tiles of the board new). The
rotation and the hand of the player are thread-local values (`layout::rotation`,
`layout::right_handed`). A new thread starts with the default value: the portrait layout of a
left-handed player. Thus each stripe thread put the cells and the lines at the portrait position,
and the copy put only the part of them that is in the landscape board area. The portrait screen was
correct because its position is the default value. The player does not see this bug at the first
drawing of a portrait screen; the owner saw it at the turn, when the screen is drawn again.

Fix: each stripe thread gets the rotation and the hand of the drawing thread before it draws.
`tests/render.rs::the_board_is_at_its_position_in_each_orientation` examines the four thick borders
of the board in portrait and in landscape. It failed before the fix ("Landscape: no board border at
(418, 809)") and passes after it.

`tests/render.rs::a_turn_from_portrait_to_landscape_shows_the_landscape_screen` examines the turn
itself: it draws the portrait screen, gives the turn (the new layout, `App::rotate`, `Presenter::reset`
and the present calls of the main loop), and takes a screenshot of the framebuffer before and after
the turn (`target/screenshots/screen-portrait.png` and `target/screenshots/screen-landscape-after-turn.png`).
It fails before the fix ("no board border after the turn at (418, 809)") and passes after it. The
screenshots show the complete screen, thus they also show a missing drawing after a turn.

Examination on the computer, 2026-11-03:

- `cargo test`: all tests pass, except `tests/install.rs::arrow_keys_select_a_version_in_a_terminal`:
  the computer has no free pty device ("out of pty devices"). This test does not touch the screen
  and it fails with and without this change.
- `cargo run --release --bin snapshot -- DIR`: the portrait states agree with `images/` as before.
  The landscape states (`landscape-*.png`, not in `images/`) show the board at the center in the
  same way as `layout.rs` gives it.
- The output hashes of the bench do not change: the bench draws the portrait screen for a
  left-handed player, thus each stripe thread had the correct values before the fix also.

REQUIREMENTS.md and INVARIANTS.md do not change: R§3.3 and NAV-10 already give the landscape layout
and the position of the board. The design images do not change (no landscape game screen is an
image).

Device test, 2026-11-03 (USB 10.11.99.1): passed. The owner started the development build from
AppLoad, opened a game with a puzzle, turned the tablet to landscape and back, and tapped a cell and
wrote a number. The board shows at the center and the puzzle is complete in landscape, and the input
operates in both orientations. The checksum of `dist/sudoku/sudoku` agrees with the file on the
tablet. Bead remarkable-sudoku-j46 is closed.

### Owner request: an x in the micro-cell for a refused pencil mark, 2026-11-04

The owner asked: "whenever some attempts to make an invalid pencil mark (a peer already has that
number, it should flash an `x` icon, similar to have unrecognized writing shows a `?` for a moment
before it disappears". Beads issue: remarkable-sudoku-5db. The change is now in REQUIREMENTS.md
(§3.2, §3.3, §4.1, §5.1, §5.2, §6.1, §7, §9, §9.1, §9.2, §12, §14.3, §14.4, §15, §17). The new image
is `images/screen-invalid-mark.png`.

Examination on the tablet, 2026-11-04:

| Prototype | What the owner examined | Result |
|---|---|---|
| First | An x at the center of the full cell, with the marks of the cell hidden | Rejected. The owner said: "the x should only be in the micro-cell". |
| Second | An x only in the micro-cell of the digit. The other marks and the grid stay visible. | Approved. |

The device results are only these two prototype results. The items below are decisions and rules.
They are not results of a device test.

Owner decisions on the edge cases, 2026-11-04:

1. Tap on a hidden mark that exists. The cell has the mark d, a peer has the number d, and the player
   taps d. The tap has no effect, and the x shows.
2. Setting off. When "Hide pencil marks automatically" is off, the tap adds or removes the mark, and
   no x shows.
3. The "?" and the x together. They can show at the same time in different cells, each with its own
   time. Example: a scribble waits in cell A, and the player does a refused tap in cell B. The app
   reads the ink first, thus the "?" shows in A, and the x shows in B.
4. The x and writing. While the player writes, screen redraws wait (R§14.4). Thus the pixels of the x
   can stay on the screen, under the ink of the player, until the writing stops: approximately one
   second after the last stroke. The owner answered "Acceptable". The state of the x goes at the
   press. The pixels go when the redraw occurs. The "?" has the same path.
5. Manual. Add one sentence about the x to page 5 (helpers), in portrait and in landscape. The text
   is: "If you tap the micro-cell of a hidden digit, the tap has no effect and a small x shows there
   for a moment."

Exact values, 2026-11-04:

- Two straight lines with round ends. Each line goes from -10 px to +10 px on the two axes around the
  center. Thus the size is 20×20 px between the line centers, and the visible extent is approximately
  23×23 px.
- Line width 3 px. Color `gray15`, the same as the pencil marks.
- The center is the anchor point of the pencil mark of the digit: 24, 74 or 122 px from the cell
  origin on each axis, at the keypad position of the digit.
- Time: the same value as the "?" (1000 ms).

Rules that follow from the approved behavior. The owner did not decide them one at a time:

- An x implies all of these: the game screen shows, no panel is open, the game is not complete, its
  cell is empty, "Hide pencil marks automatically" is on, a peer has its digit, no highlight shows and
  no ink waits. It is never on the same cell as a "?".
- The release of a control that does an action removes the x, as it removes the highlight. It removes
  only the x, not a "?" in a different cell. The rules of the "?" do not change.
- Completion, a change of screen, a new game and the update dialog remove the x.
- At a turn of the tablet, the x stays on the same cell and digit for the rest of its time. A tap in
  progress at the turn makes no x.
- A first press that the app ignores in the guard time after "< Back" (NAV-16) does not remove the x.
- With the micro-cell lines set to None, the x shows with no grid.

Owner decision, 2026-11-05: in the guard time after "< Back", a second finger on a toolbar button
does not operate the button, but it removes the x and the "?", as at all other times. The guard-time
exception applies only to a first press.

Rejected value, 2026-11-04:

- An x at the center of the full cell: 56×56 px, line 8 px, `gray40`, the marks of the cell hidden, at
  the position of the "?". Reason: the x must be only in the micro-cell.

### Owner request: a color for completed groups, a stronger palette, a wider Cell colors panel, 2026-11-05

The owner asked: "whenever a row/column/box is fully filled out without any errors, the entire
row/column/box should change its background to green (which is customizable via the paint bucket
menu). because we're adding another colour selection, we should probably make the menu wider to
support all of the different selectors. especially important on landscape". Later requests during the
trial: "can you make the colors less opaque so that they are easier to see on the tablet. all of
them"; "make the your numbers also a touch darker still"; "There should be an option to turn it off."
Beads issue: remarkable-sudoku-vms. The change is now in REQUIREMENTS.md (§2, §3.1, §3.3, §4.1, §4.2,
§5.2, §6.2, §6.3, §6.4, §7, §8.3, §9.1, §9.2, §10, §12, §14.2, §14.3, §15, §16, §17). The new images
are `images/color-menu-landscape.png` and `images/screen-completed-group.png`.

Examination on the tablet, 2026-11-04 and 2026-11-05 (the prototype). The owner approved these items:

| Item | What the owner examined | Result |
|---|---|---|
| Completed groups | A row, a column or a box that is full with no clash gets the color. A clash or an erasure removes it. | Approved. |
| Solved board | At completion, the groups lose the color, and the board shows the normal colors. | Approved. A solved board in the completed color is rejected. |
| White in the Completed groups section | White means "no completed color". A starting number keeps its starting color. | Approved. White that paints the group white is rejected. |
| Switch | "Color completed groups" in the Settings panel, default on. The Settings panel stays 400 px wide. | Approved. |
| Cell colors panel | Five sections in two columns, 800 px wide, y 112 to 1192, in portrait and in landscape. It covers approximately 27 cells in portrait and 24 in landscape. | Approved. The owner accepted this size. |
| Palette | The values in R§15, on the board and in the panel. | Approved. |
| Default highlight | Cyan (was Green), on top of the green of a complete group. | Approved. |
| Digits of the player | `gray18` (`#2E2E2E`, was `gray35`). Starting digits stay black. | Approved. |

The device results are only these prototype results. The items below are decisions, measurements and
review results. They are not results of a device test.

Decisions on the approved behavior, 2026-11-04 and 2026-11-05:

1. "No errors" means "no clash". The app does not compare numbers with the solution. Thus a group with
   an incorrect number and no clash is complete.
2. A cell of a group that clashes with a cell outside the group also stops the group.
3. A group with only starting numbers that is full at the start of a game gets the color from the
   first frame. There is no exception for it. Measured: 19 of 300 Easy puzzles; no puzzle in the other
   modes.
4. Settings file of an earlier version: R§14.2 gives the steps.

Review finding, not an owner decision: the prototype did not send an earlier settings file that breaks
COL-5 to the defaults. R§14.2 now tells this. The rule applies PER-5 to an earlier settings file. Its
source is PER-5 and the review finding. The owner did not examine it on the tablet.

Palette, 2026-11-04 and 2026-11-05:

| Color | Old value (rejected: too faint) | Approved value |
|---|---|---|
| Gray | `#C8C8C8` | `#AEAEAE` |
| White | `#FFFFFF` | `#FFFFFF` |
| Yellow | `#FFE680` | `#FFD740` |
| Blue | `#AFC4F5` | `#408CFF` |
| Red | `#F0A3A3` | `#FF8C8C` |
| Green | `#B5DF9E` | `#48D630` |
| Cyan | `#A6E0EE` | `#6FCFE6` |
| Magenta | `#DCB3E6` | `#E650E0` |
| Orange | `#F7C594` | `#FF9A1F` |

Rejected values:

| Item | Value | Reason |
|---|---|---|
| Red | `#EC7A7A` | Too intense. |
| Red | `#F09292` | Not sufficiently red. |
| Red | `#FF7F7F` | Not sufficiently light. |
| Red | `#FF5C5C`, `#FF3B3B` | Too dark. |
| Red | `#FF9C9C`, `#FF7C7C`, `#FFACAC` | Tried. `#FF8C8C` is the selection. |
| Blue | `#7FA3F0` | Too gray. |
| Blue | `#62A0FF` | Not sufficiently blue. |
| Green | `#8FCF6C` | Too gray. |
| Green | `#6CD650` | Not sufficiently green. |
| Magenta with Orange | `#C98CDB` with `#F5A45C` | Too similar. |
| Digits of the player | `gray35`, `gray25` | Not sufficiently dark. |
| Cell colors panel | One column with five sections | It does not fit in landscape (5 × 360 px). |
| Solved board | The completed color on a solved board | The board shows the normal colors at completion. |
| White in the Completed groups section | White paints the group white | White means "no completed color". |

Items to examine on the tablet at the final build. The computer cannot prove them, and the owner has
not examined them at that build:

- The screen updates when one input changes the color of many cells.
- Fast writing in the next cell while a group changes color.
- An eraser stroke over many cells.
- The stronger colors behind ink that starts on a filled cell. A Pen area on a color can give ghosts.

Owner decision, 2026-11-05: the circular arrow of "Check for updates" has the look of an image from
the owner: the arc goes from the upper right (330 degrees) clockwise to the top, and a 7 px arrow head
points to the right, with its tip 5 px to the right of the top. A dev build has no update row, thus
the owner examined the icon in a snapshot of the Settings panel, not on the tablet.

Rejected values, 2026-11-05:

- The old icon: an arc from 0 to 255 degrees, a gap, and an 8 px arrow head on the circle at 330 degrees.
  Reason: the owner gave an image with a different look.
- The tip of the head at the top of the circle, and 2.5 px to the right of the top. Reason: the head
  must be farther to the right.
