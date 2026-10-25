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
| Writing | The handwritten 5 and 8 of the owner were read correctly. The 5 removed the mark 5 of a peer (MRK-4). |
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
| The panel skips pixels of an update that overlaps an update in progress. The ink had gaps until the next update of the area. A screenshot showed solid ink in the image of xochitl; a test with a grid of black dots showed that gray pixels are not the cause. | 400 ms after the last ink update, the app sends the full ink area again (`INK_REPAIR_DELAY`). The gaps fill in. |

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
| Analytics summary, landscape | The screenshot is the same as the app's drawing of the device's results database (mean difference 0.01). The games list in two columns is examined against `analytics-games-landscape.png` (`tests/render.rs`). |
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
