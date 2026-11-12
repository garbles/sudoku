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

Since 2026-11-07, the ink of the pen, the pencil marks and the x of a refused tap have no
anti-aliasing, in the app and in the images (REQUIREMENTS.md §15). Since 2026-11-08, the board lines
and the micro-cell lines also have no anti-aliasing: the micro-cell lines are a pattern of `gray15`
and white pixels with the darkness of the selected gray, and ImageMagick draws the same pattern pixel
for pixel (REQUIREMENTS.md §4.1). Since 2026-11-08 (later), no drawing of the app has anti-aliasing,
but the design images keep the anti-aliasing of ImageMagick for the other items (the entry
"no anti-aliasing" of 2026-11-08, below).

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
| Font `/usr/share/remarkable/webui/reMarkableSans.woff2` | Loads in 19 ms. The screen drawn with it agrees with `images/pro/screen-in-progress.png` (0.135 % of pixels differ, all at edges). |
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
- `images/pro/screen.png` stays as the image of the closed menu. It is not the start state now.
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
is `images/pro/screen-invalid-mark.png`.

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
are `images/pro/color-menu-landscape.png` and `images/pro/screen-completed-group.png`.

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

---

Owner decision, 2026-11-05: the color picker `Sudoku (colors)` is a separate development app, to select
the palette colors on the real display. The owner examined it on the tablet and approved it.

- All 9 palette colors stay visible, each with an empty cell, a cell with a number of the player and a
  cell with a starting number. One set of R, G and B sliders changes the selected color.
- A change refreshes only the cells of that color when the pen or the finger lifts: black, white, then
  the color, with 500 ms for the black step and for the white step.
- The colors stay between starts. A reset button sets the colors of the code again.

Rejected, 2026-11-05:

- A Cargo feature that selects the app in `src/bin/sudoku.rs`. Reason: the owner did not find the source
  clear, and asked for a separate `bin`.
- A refresh of all preview cells for each change. Reason: only the color that changes must refresh.
- A new start with the colors of the code and a file `history.jsonl` with a line for each change. Reason:
  the owner selected saved colors with a reset button in place of this.

---

Owner decision, 2026-11-05: Red changes from `#FF8C8C` to `#FF8E8E`, and the numbers of the player change
from `gray18` (`#2E2E2E`) to `#1E1E1E`. The owner selected the values in `Sudoku (colors)` on the tablet,
and examined them in a game on the tablet (dev build `dev/colors`). The other palette colors do not
change.

Rejected, 2026-11-05:

- The number color `#001E1E`. The owner changed it on the tablet before the approval. This value was in
  the first pull of `colors.json`.

### Owner request: a Checkpoints panel, 2026-11-06

The owner asked: "a checkpoint / save state / snapshot feature where i could take a snapshot, continue on
with my puzzle, and then at any point apply my snapshot as a new action which replaces the entire state of
the puzzle." Later, on the tablet: "The checkpoints don't need a title and so the pop out doesn't need to
be so wide." Beads issue: remarkable-sudoku-jpv. The change is now in REQUIREMENTS.md (§2, §3.1, §3.3,
§4.1, §5.1, §6.1, §6.3, §6.4, §7, §8.1, §8.3, §8.4, §9, new §9.3, §11, §14.3, §15, §16, §17).

Examination on the tablet, 2026-11-06 (the prototype). The owner selected the panel design from 10
proposals, and approved the prototype on the tablet. These items have the owner approval or an owner
decision:

| Item | Result |
|---|---|
| Bookmark button in top slot 5, panel of 600 px at y 560, no title in a row | Approved. |
| Save, Restore as one new undo item, delete with no dialog | Approved. |
| A complete puzzle | Owner decision: the panel is disabled, and the bookmark is gray. In the approved prototype, a tap on the gray bookmark changes nothing, also when a different panel is open: that panel stays open (an exception to NAV-12). |
| A tap on undo or redo with the panel open | Owner decision: keep two taps. The first tap only closes the panel. |
| A press in the panel in the 0.5 s after a delete | Owner decision: it changes nothing. Thus a fast double tap does not delete the next row. |

The device results are only these prototype results. The items below are review results and measurements.
They are not results of a device test, and the owner did not examine them on the tablet:

- CKP-2: a difference only in hidden marks shows "Restore". A Restore then adds an undo item with no
  visible change.
- CKP-4: the rules of the list. CKP-5: the validity of the checkpoints and of the Restore items at load.
- CKP-6: each change of the list stops the other pen or finger gesture.
- The panel covers no toolbar button, close button, meter or "Completed in", and it has no waiting ink and
  no stroke in progress.
- Compatibility: an earlier version cannot read a saved game with the new undo item of a Restore. Thus an
  update to an older version loses the game. A preview build can update to an older stable version.
- Long clock: from the font metrics with no kerning, "99:59:59 played" is 183 px and "100:00:00 played" is
  197 px. The space to the button is 191 px. The owner has not given a rule for this (R§16).

Rejected values, 2026-11-06:

| Item | Value | Reason |
|---|---|---|
| Row of a checkpoint | A title "Checkpoint N" | Not necessary. |
| Panel width | 800 px (x 708–1507) | Too wide without the title. |
| Complete game | A choice on the small boards | The owner disabled the panel on a complete game. |

Items to examine on the tablet at the final build. The computer cannot prove them, and the owner has not
examined them at that build: open the menu, tap the bookmark below redo, tap Save checkpoint, write numbers
and marks, open the panel and tap Restore, tap outside the panel, tap undo, double-tap a bin, and complete
a puzzle (the bookmark is gray).

### Recognizer, fourth version: the examples in both pen directions, 2026-11-06

The owner asked: "can you update @src/recognizer.rs so that it calculates all symbols both forward and
backward of their paths?" Beads issue: remarkable-sudoku-d4i. The change is now in REQUIREMENTS.md (§5.1,
§16). No screen, panel, image, setting, saved value or input changes.

Examination on the tablet, 2026-11-06 (the prototype, version `dev/ink-both-ways`). The owner approved the
behavior below and these values. This record has no separate result for each written digit.

| Item | Result |
|---|---|
| Each example, in all classes and sources, and each copy of a digit example, is stored as written and fully reversed | Approved. |
| The ink of the player is not reversed, and the app calculates its features one time | Approved. |
| The app calculates the example features at each start, in the background, in memory only | Approved. |
| Acceptance factor 1.4 | Approved. No change. |
| Test limit for other characters rejected: 90.0 % | Approved. |
| Test limits for digits: 97.5 % correct or more, 1.2 % wrong or less. Target of R§5.1: 96 % correct, 1.2 % wrong | Approved. No change. |

This replaces the separate reversal of the typeface shapes (third version, above). The older sections stay
as a record.

Measured values on the computer (macOS, one run). The examples of writers 1 to 40 read writers 41 to 60. Each
row has 360 digits and 3000 other characters.

| Ink | Digits correct | Wrong digit | Others rejected |
|---|---|---|---|
| Usual direction, before the change | 98.6 % (355) | 0.6 % (2) | 90.8 % (2723) |
| Usual direction, after the change | 98.6 % (355) | 0.6 % (2) | 90.5 % (2715) |
| Fully reversed ink, before | 69.4 % (250) | 28.1 % (101) | 18.7 % (561) |
| Fully reversed ink, after | 98.6 % (355) | 0.6 % (2) | 90.5 % (2715) |
| Only strokes, or only order reversed | 98.6 % (355) | 0.6 % (2) | 72.6 % (2177) |

The older values 97.8 %, 0.8 % and 91.3 % in the sections above were already old before this change. For ink
in the usual direction, this change gives no gain for digits and a small loss of rejection. The gain is for
reversed ink.

Cost on the computer (macOS, one run), before and after:

| Item | Before | After |
|---|---|---|
| Examples | 20 124 | 40 122 |
| Feature data | 45 MB | 89 MB |
| Mean reading | 0.79 ms | 1.50 ms |
| Start calculation | 47 ms | 78 ms |

Bench on the computer (macOS), A/B with the old code on the same day. The start calculation of this bench is
not the same value as the one run above:

| Item | Before | After |
|---|---|---|
| Mean reading | 797 us | 1515 us |
| Start calculation | 29.6 ms | 57.1 ms |
| `draw` | 449 us | 444 us (no change above the noise) |
| `input_to_pixels` | 642 us | 627 us (no change above the noise) |

Cost on the tablet (Paper Pro, 2026-11-06). `sudoku --self-test`, the final dev build against the old code at
ec28c13, alternated, 15 runs each, one ink for the reading, dev profile:

| Item | Before | After |
|---|---|---|
| Ready time, normal runs | Approximately 320 ms | Approximately 590 ms |
| One reading, normal runs | Approximately 5.3 ms | Approximately 9.2 ms |
| Runs with a ready time over 1 s | 6 of 15 (maximum 1.99 s) | 8 of 15 (maximum 3.32 s) |
| Runs with one reading over 20 ms | 5 of 15 (maximum 159 ms) | 6 of 15 (maximum 368 ms) |
| Median ready time of all runs | 327 ms | 1372 ms |
| Peak memory of the process (VmHWM, one run each) | 145 MB | 235 MB |

Slow runs occur on both builds, and the cause is not known. Because more than half of the runs after the change
are slow, the median of all runs is 1372 ms, not the normal value. The reading starts at the pen lift, and the
recognition pause is approximately 0.5 s.

The last tablet bench before this change (commit c3b03d9) gives 12.2 ms for one reading and 314 ms for the start
calculation. That bench is a different measurement. The other tablet values in this document (1.7 s, 14 ms) are
history from an older version.

aarch64 Linux: `make test-linux` (musl, release and debug) passes. This includes the accuracy tests and the rule
that each ink and its full reversal give the same result on 3360 inks. The test did not print the accuracy values
on aarch64, thus this record gives only that the tests pass with their limits.

Review results. These are not results of a device test:

- Writers 41 to 60 give the same result for the ink and its full reversal on all 3360 test inks. This is
  measured on this set, and it is not proved for all ink. Thus it is a test on this set, not an absolute rule.
- Partly reversed ink is not in the rule. For only the stroke directions or only the stroke order reversed,
  rejection is 72.6 %. Ink with only some strokes reversed is not measured. The owner knows this and approved
  the change.
- A reading that comes before the end of the start calculation waits for it, and this time is now longer. The
  behavior is the same.
- The accuracy test also uses the owner examples, thus a corpus addition can change its result.

Rejected values and alternatives, 2026-11-06:

| Item | Value | Reason |
|---|---|---|
| Test limit for other characters rejected | 90.5 % | The measured value is 90.5 %, thus there is no margin for a corpus addition. |
| Direction of the ink | Reverse the ink of the player at write time, and keep only forward examples | Considered, not selected. The owner kept the approved build. |
| Size of the examples | Remove near-duplicate examples | Measured, not selected. The owner does not want a follow-up issue. |
| Size of the examples | Remove the reversed copies as "duplicates" | A backward example is typically 5 to 30 times farther from its nearest forward example than two forward examples are from each other. |

Items to examine on the tablet at the final build. The computer cannot prove them: close Sudoku and open it
again from AppLoad, write digits in the usual direction and from the bottom up, write a letter or a scribble
(the app shows the "?"), and write quickly in many cells (the ink and the response must stay fast).

### Owner request: the ink pause, ink on filled cells, the highlight, hand touches and the ink start, 2026-11-07

The owner asked, in this order:

1. "can you please bump the ink delay from 1s to 1.5s as i'm finding that 1s is not long enough. the timer
   should only start after the pen is no longer touching the tablet." Later: "now that we know its set from the
   appload folder, can you make it 1s?"
2. "i can ink on completely squares which i do not want. it is visible when i tap a number to highlight all of
   them on the screen. should not show ink in this case"
3. "touching my hand on the screen should not remove the number highlight. only pen strokes should remove it.
   toggling the menu or selecting options should not remove the highlight."
4. "doing anything while a number is highlighted triggers an immediate re-render to remove the highlight. for
   example, 7 is highlighted and then i try to write something while it's highlighted, or 7 is highlighted and
   then i tap to highlight 8. in those cases we should wait until the action is completed before we re-render.
   i.e. batch the re-renders together instead of one after another"
5. "i see that when i write multiple numbers (in separate cells) there is significant lag on the later numbers"
   (the owner said: the start of the ink).

Beads issue: remarkable-sudoku-lm1. The change is now in REQUIREMENTS.md (§3.2, §3.3, §5.1, §5.2, §6.2, §7, §9,
§9.1, §9.2, §14.4, §15, §16).

Examination on the tablet, 2026-11-07 UTC (the prototype, version `dev/highlight-keep`). The owner approved the
behavior in the requirements. This record has no separate result for each item, except where it tells so.

| Item | Result |
|---|---|
| Recognition pause of 1 s from the pen lift, one fixed value in the source. Redraw wait of 1 s | Approved. |
| No ink and no pen area for a touch that starts on a filled cell | Approved. |
| The list of items that remove a highlight, and the list of items that do not (R§6.2) | Approved. |
| One redraw for each action. Two updates remain in two known cases (R§5.2) | Approved. The owner accepts the two cases. |
| A touch that starts less than 150 ms after a pen lift has no effect | Approved. The owner knows the risk: a hand touch more than 150 ms after a pen lift, on a toolbar button, still operates it. |
| A finger press does not read waiting ink and does not stop the writing | Approved. |
| The pen area starts at the pen down while the player writes | Approved. |
| A tap on an empty cell while the player writes starts the area, and its mark or x comes after the area stops | The owner tried taps and did not reject this. |

Measured on the tablet (input log, 2026-11-07): the tablet sends a touch from the hand approximately 97 ms after
each pen lift, and that touch ends a few ms before the next pen down.

Measured with the update log of xochitl:

| Measure | Before | After |
|---|---|---|
| Pen down to first ink, later strokes | 73 to 164 ms | 52 to 56 ms, pen waveform |
| Area start after pen down, later strokes | 58 to 241 ms | 2 to 12 ms |
| First stroke of a session | Not compared | Approximately 135 ms |

Time line for one number: the read at 1 s, the digit goes to the screen approximately 45 ms after that, and the
display completes it at approximately 1.45 s. The last value is an estimate from the 400 ms that the app gives a
screen update. It is not measured on the panel. The owner approved a new bench baseline for the computer and
Linux (2026-11-07). There is no tablet baseline.

History of the examination:

- The owner's first request said "1s". The manifest set the pause to 0.5 s, and it overrode the source. The log
  showed the read at about 505 ms and the redraw at about 1018 ms. That is the "1 s" that the owner saw, and the
  digit was fully visible at about 1.4 s.
- A first prototype changed only the source constant. The owner tested it without the new pause, and the agent
  wrongly reported 1.5 s. A log found the override.
- The pen area was off on the tablet: the patch file was newer than the start of xochitl. A restart of xochitl
  turned it on. This was the cause of a part of the ink lag. The app gives only one line in the log for this state.
- AppLoad keeps the manifest in memory until xochitl restarts (observed: the running process had the old value
  until the restart).
- One of the xochitl restarts (`xovi/start`, 2026-11-07 02:20 UTC) gave a segmentation fault of xochitl and a
  reboot of the tablet. The cause is not known. Four other restarts in the same session did not do this. After the
  reboot, xovi, the patch and AppLoad operated.
- The update log showed no backlog in xochitl: each app update gets to the panel call in approximately 8 ms. The
  theory that refreshes build up has no support at that level.

Review results. These are not results of a device test:

- A stroke on a filled cell removes the highlight in the state, but the pixels can stay up to 1 s. A tap on that
  number in this time makes a highlight, and the player can see this as a failed removal. The owner did not ask
  for a change.
- A press on an empty cell where a finger Redo fills that cell in the same touch: the pen area may start on a
  filled cell. This is not proved visible.

Rejected values and alternatives, 2026-11-07:

| Item | Value | Reason |
|---|---|---|
| Recognition pause | 1.5 s | The owner rejected it after the owner saw that the digit is fully visible approximately 0.45 s after the read. |
| Recognition pause and idle time | 0.55 s (a digit that is fully visible at 1 s) | Offered, not selected. It gives less time for a second stroke. |
| Recognition pause | 0.75 s | Offered, not selected. It gives less time for a second stroke. |
| Highlight | It comes back on return from another screen | Not selected. |
| Hand touches | Leave them as they are | Not selected. |
| Source of the pause | Keep it in the manifest | Not selected. |

The older sections stay as a record. Their pause of 0.5 s and their highlight rules are history.

### Owner request: only the pen operates the close button, 2026-11-06

The owner asked: "only allow the app close button to be triggered by the pen".

The owner examined the dev build `dev/close-pen-only` on the tablet and approved it: a finger on the close
button does not close the app, the pen closes the app, and a finger on the close button with an open panel
only closes the panel.

Details that follow from the existing rules, not examined one by one on the tablet:

- The eraser end is a part of the pen, thus it also closes the app.
- A finger press on the close button removes the "?" and the x, as each press does (QST-4, MRK-15).
- The close button of the color picker (`Sudoku (colors)`) does not change. It is a separate development tool.

The owner then asked: "yes please update the manual. include a note that states that you can only close it with
the pen so that resting your hand on the device doesn't close it by accident". Page 1 of the manual now has the
note "Only the pen closes the game. Thus your hand cannot close it by accident." below the close button. The owner
examined the page on the tablet, in portrait and in landscape, and approved it.

### Owner request: the re-render policy and the pen area at each pen down, 2026-11-07

The owner asked (autonomous work, no questions, no install on the tablet): "lets refactor how
re-renders happen." Immediate: "adding/removing a pencil mark", "writing the pen ink", "opening/closing
the menu or sub menus". Batched until the 1 s idle timeout: "writing numbers in cells". Highlights "as
it currently is". "re-rendering should _not_ block ink writing." "the ink must be smooth from the
moment the pen touches the cell ... start the pen area at every pen down on an empty cell, also for a
tap". "a pencil mark tap must be fast: the mark or the x goes to the screen at the pen lift with the
pen waveform, inside the pen area, not after the area stopped". "after a tap, only the micro-cell of
the mark may refresh, not the entire cell. xochitl refreshes the full pen area when the area stops or
changes size, so do not try a smaller area at the lift". "the ink of a tap must go away when the mark
shows". Beads issue: remarkable-sudoku-c2m. The change is in REQUIREMENTS.md (§5.1, §14.4) and
INVARIANTS.md (INP-2, INP-24).

**The owner has not examined this change on the tablet.** No item below is a device result.

What changed:

| Item | Before | After |
|---|---|---|
| The start of the pen area | The first stroke of a session: when the pen left the micro-cell. A tap at that time: no area. While the player wrote: at the pen down. | Each pen press on an empty cell: at the pen down, also a tap and also the first stroke. |
| The area after a pen lift | It stopped when no ink waited and the pen did not write (a tap: at the lift). | It stays on its cell. It moves at a press on a different empty cell. It stops only before a redraw that needs the UI waveform in its cell (a number, a "?", other micro-cell lines, a panel, a dialog or a different screen over the cell), when its cell becomes filled, or at a turn. |
| The mark or the x of a tap | After the area stopped (45 ms), with a refresh of the full cell that showed the ink dot of the tap. | At the pen lift, inside the area, with the pen waveform, only the changed pixels of the micro-cell, together with the removal of the ink dot. A tap shorter than 45 ms: the mark waits until the patch applied the area. |
| The end of the x | A redraw with the UI waveform. | Inside the area, in the micro-cell, if the area is still on that cell. |
| The number of a written cell | After the writing stops: the area stops, 45 ms, then the redraw. | The same. |
| The menu and the panels | At once. | At once. A panel over the cell of the area: the area stops first (45 ms). |

How the area avoids a refresh of the full cell after a tap: the area does not change at the lift.
xochitl refreshes the old area and the new area of the pen area at each start, stop or move, also
when no pixel in them changed, and the ink patch cannot avoid this. Thus the only way to avoid the
refresh is to change the area as rarely as possible. The refresh of the full cell still occurs when
the area moves to the next cell or stops before a number: these refreshes existed before this change.

The rule for the stop before a redraw: the pen waveform shows black and gray15 on white (the ink, the
pencil marks and the x). A number with its color, the "?" (gray40), other micro-cell lines, and a
panel over the cell need the UI waveform. `render::FrameRecord::cell_change_shows_with_pen` compares the
board tile of the cell on the screen with the tile in the frame, and it looks for a drawing that is
not ink over the cell since the last copy of the frame to the screen.

Tests changed because the behavior changed: the first stroke of a session and a tap start the area at
the pen down (`tests/inv_input.rs`, `tests/behavior.rs`, `tests/properties.rs`, `src/present.rs`); the
mark of a fast tap waits for the area (`src/present.rs`); a tap with the pen area not in use
(`tests/inv_render.rs`: the assertion that the app starts no area is removed). `Presenter::reset` takes
the time (`tests/render.rs`).

Not done: the bench baselines are not saved (the owner asked that the bench runs only on request). The
output hashes of the updates of the bench script change with this change. `make test-linux` passed.

Items to examine on the tablet: the first ink of a stroke (no lag at the start of a stroke, also the
first stroke of a session); a tap on a mark and on a hidden mark (the mark or the x at the lift, only
the micro-cell refreshes, no ink dot stays); a tap, then writing in a different cell (the old cell
refreshes when the area moves); the end of the x; a tap, then a panel that covers the cell; a number
after a tap in the same cell; a turn of the tablet with the area on; the update log of xochitl for the
`pen (ui)` updates of the marks.

### Owner request: a screen with the third-party licenses, 2026-11-07

Request: a screen with all third-party dependencies and their licenses, collected by a program.

Decisions of the owner (before the prototype): the entry is a row of the Settings panel; the screen
is a list with pages, and a tap on a row shows the full license texts; the list contains only the code
in the tablet program (no proc macros, no build or dev dependencies).

The owner examined the prototype on the tablet (`dev/licenses`) and approved it after two changes:

- Rejected: a second line in each row of the list with the SPDX license. The owner asked to remove
  it. The license shows only on the text page.
- Changed: on the text page, ` OR ` in the license expression shows as ` / `.

Added after the review of the prototype: an entry for the UJI Pen Characters data (CC BY 4.0 requires
attribution). The owner asked for light tests only, with no team (`tests/licenses.rs` and the changed
Settings tests).

Follow-up (remarkable-sudoku-ed6, same day): the reference states and the images `licenses.png` and
`licenses-text.png` (and landscape), drawn by `scripts/analytics.py` from `assets/licenses.json`; the
page-range checks LIC-2 and LIC-3 in `invariants.rs`; a test of a turn on the two screens; manual page
7 ("Turns the helpers on or off. Shows the version and the licenses."); the bench baselines. The
license text page has much more text than the other reference images. The anti-aliasing of the glyph
edges of ImageMagick and tiny-skia is different, thus its mean difference is 1.72 (limit 1.5). That
image has higher limits (mean 2.5, 1.5% of the pixels) in `tests/render.rs`. The checks of the pixels
away from the edges are the same as for all images (no difference).


### Owner request: the result card of a completed puzzle, and a darker secondary gray, 2026-11-07

Request (design review): keep the result card of design 1, but remove the Statistics button, the New
game button and the "New best" pill. Add new game buttons that select an easier, the same or a harder
level. The owner selected design D ("Split card") and the rule "the card never touches the toolbar".

The owner examined the prototype on the tablet (`dev/result-card`, epic remarkable-sudoku-3ow) and
approved it after two changes:

- Rejected: **Same** with a black fill and white text (as in the mocks). The owner said "do not
  highlight Same". The three buttons are equal: 2 px black frame, white inside.
- Rejected: `gray40` (`#666666`) for secondary text. "Completed in" was hard to read on the tablet.
  The owner asked for a darker gray in all interfaces, also the manual pages: `gray25` (`#404040`).
  The micro-cell line setting 40 (COL-7) is a setting, not a text color, and does not change.

Decisions of the owner (2026-11-07):

- Landscape: the card is a column of 322 px at the side opposite the toolbar. Only the column moves;
  its contents are not a mirror image.
- Rejected: the landscape time at Medium 96. "1:00:00" does not fit in the 322 px column. The time is
  always Medium 72 in landscape.
- One tap on a level button starts the game, with no confirmation. The pen tip, the eraser and a
  finger operate the buttons. After a level button, the menu is closed.
- Only the puzzle of **Same** is made in the background. Easier and Harder are made at the tap.
- An open panel can cover a part of the card (for example the portrait New game panel). Accepted.

Rejected in the design rounds: a Statistics button, a New game button and a "New best" pill in the
card.

Findings of the review: the prototype started the early Same puzzle only at the next input. It must
start at the completion, also at a completion by the reading of the ink and when a completed game
opens from the save. The comparison does not use the result of the current game (same seed). The
text of `manual-finished` "To start a new game, tap + in the menu" must change.

### Owner request: a Symmetrical option for new puzzles, 2026-11-07

Request: "lets add a `symmetrical` option for generating puzzles as most sudoku puzzles are
symmetrical. it can be toggled on and off with the new game pop up. this value is `true` for new
installs. when toggled, it should persist between new games. [...] for the result card, do not show
the option to toggle it and just carry forward whatever was used in the completed game."

The owner examined the prototype on the tablet (`dev/symmetric`, epic remarkable-sudoku-r9s) and
approved it with no change. The owner examined:

- The New game panel with three rows (slider, Symmetrical, Start new game), top at y 1822 in
  portrait. At a new install, the switch is on, and Start new game gives starting numbers that are
  symmetric under a half turn.
- Switch off, Start new game, then open the panel again: the switch is off.
- Change the switch, tap outside, then open the panel again: the change is lost.
- Complete a game and tap Same: the new game has the symmetry of the completed game.

Decisions of the owner (2026-11-07):

- Symmetric means 2-way rotational symmetry (180°). A puzzle that is not symmetric does not change
  for the same seed and mode, and the generator version stays 2.
- A symmetric Easy puzzle has 44 or 45 starting numbers. Accepted (16 of 300 sample seeds gave 45,
  because 81 − 44 is odd and the solver can need the center cell).
- In portrait with a left-handed player, the taller panel covers all three level buttons of the
  result card (before: Same and Harder). Accepted.
- The results table gets the column `symmetric` (old rows: false). Symmetric and non-symmetric games
  share the averages and the best times. The completed games list does not show or filter the
  symmetry.
- The manual page about a new game gets one sentence about the Symmetrical switch.
- Defaults of old files: a settings file with no symmetry gives on; a game file with no symmetry is
  not symmetric.

Rejected values: none.

Measured on the computer (prototype): the symmetric puzzles of all modes got to the level of their
mode, and the slowest Hard puzzle took 23 ms.

### Owner request: crisp ink, pencil marks and x (no anti-aliasing), 2026-11-07

Request: after a review of the decompiled xochitl source (`.scratchpad/xochitl-source/FINDINGS.md`),
the owner asked "can we try 1": pure black and white UI pixels, so that the display code of xochitl
gives the rectangles a faster waveform. Epic remarkable-sudoku-ol6.

**Iteration 1, rejected** (build `dev/bw-pixels`). All drawings lost their anti-aliasing, and each
gray pixel became black or white with an ordered dither (the colors stayed). The owner examined it on
the tablet and rejected it: the colored cells were washed out or had a wrong tint, and the close of
the menu left a ghost over the colored cells. The owner saw no white speckles in the digits or the
marks. Cause (inferred from the xochitl source): the display code selects a fast mono waveform for a
rectangle whose area was pure black and white before the update; a color that then appears in such
an area is driven by a mono waveform. Thus: **no drawing other than the ink, the marks and the x may
lose its anti-aliasing, and no gray may become a black and white pattern.** Do not try this again.
Correction 2026-11-08: the micro-cell lines are the exception. They show the selected gray as a
pattern of `gray15` and white pixels (the entry of 2026-11-08, below). The dots are `gray15` and not
black, because black dots gave an empty cell the mono waveform for a color that appeared in it. A
full black and white interface stays rejected.

**Iteration 2, approved** (build `dev/crisp-ink`, owner approval on the tablet 2026-11-07). Only the
ink of the pen (the strokes and the dot of a tap), the pencil marks and the x of a refused tap are
drawn with no anti-aliasing. Each pixel of these drawings is fully black (the ink) or `gray15` (the
marks and the x), or the pixel that was under it. All other drawings keep their anti-aliasing. The
colors, the grays and the geometry do not change. The owner examined:

- Writing a digit in an empty cell: the stroke has crisp edges. "The writing is significantly
  smoother" (the ink while the pen moves, and the mark, the x and the number after the lift).
- A tap on a micro-cell: the pencil mark has crisp edges. A tap on a mark that a peer hides: the x
  has crisp edges.
- The menu and a panel over colored cells, opened and closed: "Smoother, colors correct". The colored
  cells look as before the change. No ghost.

Why it helps: the ink goes to the panel in the pen area of the ink patch with the pen waveform. The
pixel conversion of xochitl for the pen waveform sends gray pixels (the anti-aliased edges) through
the color dither of the panel. Pure black (and `gray15`) pixels give a cleaner stroke. The gain comes
from the crisp pixels, not from a faster waveform.

Decisions of the owner (2026-11-07):

- The dot of a tap and the x are crisp too: "Crisp, as in the prototype".
- The design images with pencil marks or an x are made again with crisp marks and x: "Make the
  images again". The manual scripts and the manual text do not change; the "helpers" pages
  changed only because they show a scaled picture of the screen with pencil marks.
- This is the default behavior for all players. There is no setting, no switch, no environment
  variable and no file that changes it (the prototype used the test switch `bw`; the final change
  has none). It is the same in all four layouts, in the first frame, in a redraw, in the snapshot
  tool, in the bench and in the videos.

Rejected values: the full black and white interface with an ordered dither of the grays
(iteration 1, above).

### Owner request: the micro-cell grid as a dotted pattern of gray15, crisp board lines, 2026-11-08

Request: "are the grids drawn with crisp? if not, can we try doing that? i'm particularly interested
in the micro-cell grid. if we have to use dithering to achieve lighter colors then i would like to
see it". Epic remarkable-sudoku-bzk. Background from the review of the xochitl source
(`.scratchpad/xochitl-source/FINDINGS.md`): the panel has no gray particle. A flat gray pixel is a
colored dither on the panel, and a 16x16 tile with 16 or more flat gray pixels loses the clean text
path for the digits in it. Thus a lighter line must be a pattern of dark and white pixels.

**Iteration 1, rejected** (build `dev/crisp-grid`, test setting `grid = dither`, black dots). The
micro-cell lines were an ordered 4x4 Bayer pattern of black and white pixels with the darkness of
the selected gray, relative to the screen position. The owner examined it on the tablet: "Dither is
the one" (the grid), and "Correct" (the grid with the dithered swatches of the Cell colors panel).
Two problems: an empty cell was a pure black and white area, and xochitl selected its mono waveform
when a color appeared in the cell (a written digit that clashes): the clash color was washed out.
And for gray80, 70, 55 and 45 some cells showed darker lines than their neighbors: the cell is
147 px and the offsets 49 and 98 px are not multiples of 4, thus the pattern relative to the screen
cut the lines at different phases.

**Iteration 2, approved** (the same build, owner approval on the tablet 2026-11-08). The dots are
`gray15` (the gray of the pencil marks), and the pattern starts at the first pixel of each line. The
owner examined a written digit that clashes in a previously empty cell and the lines of all cells:
"Clash color correct, lines uniform". The dots look nearly black on the panel. The swatches of the
Micro-cell lines section show the pattern of their level inside the ring (a white disc with a radius
of 17.5 px, the pattern from the top left of the box of the disc); the None swatch stays a white
dot. The board lines (3 px and 13 px, black) have no anti-aliasing. Before, each board line had one faint anti-aliased pixel (value 191 of 255) on its right or bottom side (at
offset + 2 for a 3 px line, offset + 7 for a 13 px line), and the ends of the
border had pixels of 143. With the crisp lines these pixels are white: the black pixels do not change,
the faint edge pixels go away. The design images get the
same crisp board lines.

Decisions of the owner (2026-11-08):

- The micro-cell lines are the only drawing that shows a gray as a pattern (with their swatches).
  The correction of the record of 2026-11-07 is above.
- This is the default behavior for all players. There is no setting, no switch, no environment
  variable and no file that changes it (the prototype used the test setting `grid`; the final
  change has none). It is the same in the four layouts, in the first frame, in a redraw, in the
  snapshot tool, in the bench and in the videos.
- The manual pages keep their gray lines (they are illustrations); the "helpers" pages show a scaled
  picture of the screen, which changes with the board. The small boards of the Checkpoints panel
  draw no micro-cell lines. The pen area rule stays: a change of the micro-cell lines stops the pen
  area and goes with the UI waveform (REQUIREMENTS.md §14.4).

Rejected values: black dots (iteration 1: the clash color washed out by the mono waveform); the
pattern relative to the screen position (iteration 1: some cells had darker lines). Not shown to the
owner: flat gray lines with no anti-aliasing (the `crisp` variant of the test setting).

The owner also reported "the writing is very choppy only in landscape mode". The same build without
the grid pattern gave the same result, thus the grid is not the cause; a separate beads bug issue
records it.

### Owner request: crisp icons, 2026-11-08

The owner asked: "can you make all of the icons render with crisp too?" The prototype (build `dev/crisp-icons`) draws each `render::Icon` with no anti-aliasing. The owner approved it on the tablet ("Correct") and asked to skip the team procedure: the images were made again (the icon sheet `icons.png` and the screens with icons), the tests pass, and NAV-20 and the §15 row "Anti-aliasing" list the icons. The ImageMagick icons of the design images keep their anti-aliasing: the snapshot tests accept the edge pixels.

### Owner request: no anti-aliasing, 2026-11-08

Request: "as a spike, can you remove all anti-aliasing so that i can see whether it improves rendering
performance", then "lets keep that change. no more anti-aliasing". Issue remarkable-sudoku-0se. The
prototype (build `dev/no-antialias`) draws all shapes and all text of the app with no anti-aliasing.
The owner examined it on the tablet and approved it, and asked for the work without the team
procedure.

This is not iteration 1 of 2026-11-07 (build `dev/bw-pixels`, rejected): that build also changed
each gray pixel to black or white with a dither. Now only the edges change: each pixel of a drawing
is fully the color of the drawing or the pixel under it, and the colors and the grays stay flat.

Decisions of the owner (2026-11-08):

- The manual pages do not change: they are prepared images, the app copies their pixels, and they
  keep their anti-aliasing ("Keep manual as is").
- The design images in `images/` keep the anti-aliasing of ImageMagick for the numbers, the texts,
  the panels and the icons ("Keep AA images"). A test build of crisp design images
  (ImageMagick `+antialias`) agreed less with the app: the crisp glyphs of FreeType and of tiny-skia
  are different by 1 px at many edges (for example `licenses-text.png`: 6994 pixels more than 2 px
  from an edge of the image, 0 with the anti-aliased image). Thus the anti-aliased images are the
  better reference for the positions and the shapes.

The comparison limits of `tests/render.rs` changed: a maximum mean difference of 2.0 (before 1.5)
and a maximum of 1.2 % of the pixels that differ by more than 64 (before 0.8 %) for all images
except the license text page, because each crisp edge of the app differs from the soft edge of the
image. Under the veil of a dialog (`update-restart.png`, the delete dialogs of the analytics screen)
the edges have low contrast, thus these images permit 12 pixels more than 1 px from an edge
(before 2). The pixels more than 2 px from an edge stay at 0.

A crisp circle at a different x had a different edge pixel (the f32 rounding of the curves at large
coordinates): the Cell colors panel was not the same in the four layouts, and the meter of the
result card was not the same as the meter below the board. Thus the app makes each circle near the
origin and moves it by full pixels, as the switch of the Settings panel.

### Bug: choppy ink with gaps only in landscape, 2026-11-08

Owner report: "ink writing performance is very poor (choppy, with gaps) only when in landscape
mode. Portrait mode is smooth." (bd remarkable-sudoku-c4x).

Cause: the app wrote the pen area of the ink patch (`/tmp/sudoku-pen-region`) in framebuffer
coordinates (always portrait). The AppLoad source (`FBController`) turns only the drawing of the
framebuffer (`fbRotation`); the items of the window, also the `ScreenModeItem` of the ink patch, use
the coordinates of the screen as the player sees it. In portrait (rotation 0) the two are the same.
In landscape the area was on a different place of the screen (for example the cell at (418, 148) gave
the area at (1328, 420)). Thus the ink got the UI waveform, and the scheduler (`present::Scheduler`)
did not wait for the updates in progress in the cell of the ink, because it thought that the cell
had the pen waveform. Portrait at 180° had the same fault.

Fix: `Presenter::take_region_change` gives the area in window coordinates
(`layout::screen_rect`, the inverse of `layout::physical_rect`). The area is the same pixels of the
framebuffer as before, thus it stays on the 4 px grid of the panel. The ink patch does not change.
Tests: `present::tests::the_pen_region_is_in_the_coordinates_of_the_window_in_each_rotation` (it
failed before the fix in rotation 1) and
`layout::tests::screen_rect_is_the_inverse_of_physical_rect_in_each_rotation`.

Device test (build `dev/landscape-ink`, 2026-11-08): the owner wrote in landscape and reported "this change fixed the issue". The ink is smooth in landscape.

### Bug: a ghost of a pencil mark after its removal, 2026-11-08

Owner report: "pencil mark any micro cell, draw a number into a full cell, tap the pencil mark to
remove it, the pencil mark ghost is left behind" (bd remarkable-sudoku-a9u).

Sequence on the computer (`present.rs` test rig): the first tap starts the pen area on the cell A
and the mark goes with the pen waveform. The number in the cell B moves the area to B (xochitl
refreshes A with the UI waveform), and the redraw of the number stops it. The second tap on A starts
the area on A again, and the removal of the mark went at the pen lift with the area on: the pen
waveform removed a mark that the UI waveform showed. A mark that the pen waveform drew and removed
in the same area had no ghost.

Cause (inferred, not measured): the pen waveform cannot drive the colored dots of the color dither
that the UI waveform made for the `gray15` pixels of the mark (refer to the lesson "The Pen
waveform cannot drive colors" in the `remarkable-debug` skill).

First fix: the area stopped before the removal, and the removal went with the UI waveform 45 ms
after the pen lift. The owner rejected it: "the pencil mark is removed but the entire cell fully
refreshes. This is unacceptable. It should just be the micro-cell that full refreshes (if
necessary)". The stop of the area refreshes the full cell (xochitl), and the start at the pen down
also refreshes it.

Fix: a pen press on a micro-cell that shows a mark does not ask for the area while the touch can be
a tap (`App::pen_cell`). The area stays where it is (no refresh), and the removal goes at the pen
lift with the UI waveform, only in the micro-cell. When the pen leaves the micro-cell (writing), the
area comes to the cell as before. The presenter also records the marks and the x that the pen
waveform drew while the area stayed on its cell (`render::PenDrawn`). `FrameRecord::pen_change`
(before: `cell_change_shows_with_pen`) accepts a removal only of these items. Else the area stops
before the redraw: this occurs only when the area came to the cell for a different mark and the
tap removes an older mark (the full cell refreshes). Test:
`present::tests::a_mark_that_the_ui_waveform_drew_goes_away_with_the_ui_waveform` (it failed
without the change of `App::pen_cell`).

Device test (build `dev/mark-ghost`, 2026-11-08): the owner reported that the removal "is now doing
what it's supposed to do". The owner also saw the cell of a new mark refresh when the player then
writes in a different cell: the area moves, and xochitl refreshes the full area that it left (this
was also the behavior before the fix). Options were an area on the micro-cell only for a tap, or no
area for a tap. Owner decision: keep it as it is.

### Owner request: support the reMarkable Paper Pro Move, 2026-11-10

The owner has no Move. The facts come from the AppLoad source (`src/qtfb/common.h`: the framebuffer type 4
is 954×1696 RGB888; the server makes the buffer from the type that the app asks for, and it scales the
buffer to the screen), from the reMarkable SDKs of OS 3.28.0.172 (the target sysroots of `ferrari` and
`chiappa` are the same, thus the same static aarch64 build runs on the two devices) and from the product
pages (954×1696 at 264 DPI).

Owner decisions (REQUIREMENTS.md §1.1): cells of 78 px in the two orientations (first 100 px; changed
to 90 px so that the meter fits in landscape and the board keeps its physical position; then 80 px,
because with 90 px the board went 24 px under the toolbar in portrait. Options: 84 px (gap 3 px),
82 px, 80 px (gap 21 px), 75 px (the physical gap of the Paper Pro); the owner selected 80 px; on 2026-11-11, 78 px with the lines on their own pixels), toolbar slots of 96 px
with the icons of the Paper Pro, panel rows of 112 px, the Cell colors panel as three columns of two
rows in landscape, the Checkpoints panel from slot 1 in landscape, a maximum of 4 checkpoints on all
devices (first: rows of 150 px with a smaller board, rejected), a page scale of 954 / 1404, the compact
result card, new manual images. Rejected: AppLoad scaling of the Paper Pro buffer (not crisp, and the
pen area of the ink patch is at the wrong place).

Details that the implementation added, with no device test: the compact card in portrait is 24 px from
the toolbar (CMP-7); in landscape its column is 300 px wide and 120 px from the screen edge, because at
48 px it touched the close button (with cells of 90 px the board started at y 72, in the slot of the
close button). The column has the height of the board (720 px): its contents have smaller spaces than
on the Paper Pro and no "New game" heading. On
the Move, the Checkpoints panel with 4 rows covers the top of the meter in portrait (y 1312, the meter
at y 1272): a panel can cover the board and the meter there, as the owner allowed for the board.

Owner decision (after the first Move images): the portrait statistics and Licenses pages were 40 %
empty at the bottom. The extra 624 units of a portrait page now contain more rows: 18 games, 20 Licenses
rows, 57 lines of license text, and rows of 186 units in the "By difficulty" table. Rejected options:
taller rows only, more list rows only, no change.

Examination: `tests/move_device.rs` (layout rules in the four layouts, all rules of `check_app` on the
reference states, the app against `images/move/` in portrait and landscape, the manual pages pixel for
pixel) and `snapshot -- --device move`. Device test: pending (no device).

## Owner request: board lines on their own pixels, 2026-11-11

Request: "lets use the same thickness for the pro. but also lets make sure that the borders are outside of the
boxes. i see that on the edges, pencil marks are not actually centered in the microcell because the border is
overlapping". Before, the lines were at the center of the cell borders: a 13 px line covered 6 px of each cell
next to it, a 3 px line 1 px, thus a pencil mark in a micro-cell next to the border was not at the center of the
visible part of the micro-cell.

Owner decisions (AskUserQuestion): the border and the box lines are 7 px on the Paper Pro and on the Move (first
asked: 13 px was too thick on the Move); all lines, also the 3 px cell lines, have their own pixels between the
cells; the cells keep their size on the Paper Pro (147 px) and the board grows to 9 × 147 + 4 × 7 + 6 × 3 = 1369 px
(rejected: smaller cells with the same board size); on the Move the cells are 78 px (board 748 px, 7 px from the
toolbar in portrait; options were 75, 77, 78 and 79 px; 80 px gave a board 2 px under the toolbar).

Effects on the Paper Pro: the board is at x 125–1494, y 395–1764 (14 px from the open toolbar, before 37 px); the
meter and the result card move down by 23 px (card y 1813–2133). The Helpers and the Complete a puzzle manual pages
take their crops of the board from the geometry (the Helpers crop was a fixed Paper Pro rectangle). Examination:
`cargo test` (the pixel model of the board in `tests/inv_render.rs`, the black pixels of the lines in
`tests/render.rs`), the snapshot tool against the new images. Device test on the Paper Pro: pending.

