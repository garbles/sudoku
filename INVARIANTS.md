# Sudoku for reMarkable Paper Pro — State invariants

<!--
Format for scripts/invariants.py (the visualizer, `make invariants`). Keep it when you edit:
- A section is a heading "## N. Title" or "### N.M Title".
- A rule is one table row: | ID | Rule | Source |. The ID is 2 to 4 capital letters, a hyphen and a
  number. Do not use a line break in a row. Write "|" in a rule only in a code span.
- The state model is the code block between the markers state-model:start and state-model:end.
  Each line of it is one of these:
  - a type: "Name" (a capital letter, then letters), with no indent;
  - an alias: "Name = definition", with no indent;
  - a field: an indent of 2 spaces, the names ("a" or "a, b[r][c]"), then optionally an operator
    (∈, :, ⊆ or ≥) and the domain. The domain applies to all names of the line, and it has no
    comma out of braces and parentheses: write fields with different domains on different lines;
  - a continuation: an indent of 20 or more spaces. It adds to the note of the item above.
  A note is a group in parentheses at the end of a line, after two or more spaces. A type name in
  a domain, out of braces and parentheses, refers to that type.
- Each term is one item "- **name**: text" between the markers terms:start and terms:end.
- Each rule ID in the text (with the prefix of a rule) must be a rule.
`python3 scripts/invariants.py --check` examines this format. tests/invariants_format.rs runs it
(also on CI).
-->

This document is written in ASD-STE100 Simplified Technical English (STE).

This document gives the rules that must be true **in all states that the app can get to**. The rules
come from [`REQUIREMENTS.md`](REQUIREMENTS.md). You can examine each rule with an assertion in the
code, a property-based test, or a check of the saved data.

If a rule in this document and the requirements do not agree, the requirements are correct. Then
correct this document.

Each rule has an ID (for example `BRD-3`). Each rule also gives the section of the requirements that it
comes from (for example *R§4.1*). Rules that are not confirmed yet are in
[§17](#17-rules-to-confirm).

## Contents

0. [State model](#0-state-model)
1. [Puzzle and solution](#1-puzzle-and-solution)
2. [Cells](#2-cells)
3. [Pencil marks](#3-pencil-marks)
4. [Clashes](#4-clashes)
5. [Overlays: highlight, "?" and x](#5-overlays-highlight--and-x)
6. [Input](#6-input)
7. [Undo and redo](#7-undo-and-redo)
8. [Completion and the lock](#8-completion-and-the-lock)
9. [Clock](#9-clock)
10. [Screens, menu and panels](#10-screens-menu-and-panels)
    - [Checkpoints panel](#101-checkpoints-panel)
11. [Difficulty](#11-difficulty)
12. [Cell colors](#12-cell-colors)
13. [Manual](#13-manual)
14. [Statistics and the results database](#14-statistics-and-the-results-database)
15. [Saved data](#15-saved-data)
16. [Updates](#16-updates)
17. [Rules to confirm](#17-rules-to-confirm)

---

## 0. State model

The rules use this model of the state of the app. The names are examples. The code can use different
names if it has an equivalent state.

<!-- invariants:state-model:start -->
```
App
  screen          ∈ {Game, Manual, Statistics, GamesList, Licenses, LicenseText}
  game            : Game
  ui              : GameUI
  manual_page     ∈ 1..9                       (used when screen = Manual)
  list            : ListUI                     (used when screen = GamesList)
  confirm_all     ∈ {false, true}              (used when screen = Statistics: the "Delete all games" dialog)
  licenses        : (page ≥ 1, open, text_page ≥ 1)   (used when screen ∈ {Licenses, LicenseText}; not saved)
  settings        : Settings
  results         : set of Result               (the SQLite table)
  update          : Update

Game
  seed            : integer                    (from the generator; no fixed range)
  mode            ∈ 0..6
  generator       ∈ {1, 2}                     (the version of the generator; 1 only for mode ∈ 0..4)
  solution[r][c]  ∈ 1..9                       (for r, c ∈ 0..8)
  given[r][c]     ∈ {true, false}
  value[r][c]     ∈ 0..9                       (0 = empty)
  marks[r][c]     ⊆ {1..9}                     (pencil marks; kept also when hidden)
  undo, redo      : stacks of Change
  checkpoints     : list of Checkpoint         (newest first; 0 to 5 items; default empty; CKP-4)
  elapsed         : seconds ≥ 0
  completed       : bool

Checkpoint
  value[r][c], marks[r][c]                     (the numbers and all marks at the save, also the hidden marks)
  elapsed         : seconds ≥ 0                (the clock at the save)

Change = a mark | a number | an erasure | Restore { before, after }
                                               (Restore: before and after each have value and marks of all cells)

GameUI
  menu            ∈ {Closed, Open}
  panel           ∈ {None, Colors, Settings, NewGame, Checkpoints}
  highlight       ∈ {None} ∪ 1..9              (can show while the menu or a panel is open, and with waiting ink; HLT-4, HLT-9)
  question        ∈ {None} ∪ (cell, shown_at)   (the temporary "?")
  refused         ∈ {None} ∪ (cell, digit, shown_at)   (the temporary x of a refused tap; default None; not saved)
  pending_ink     ∈ {None} ∪ (cell, strokes, last_lift_at)
  slider          ∈ 0..6                       (the selection in the New game panel)
  delete_guard    ∈ {None} ∪ deleted_at        (the time of the last delete of a checkpoint, for 0.5 s; default None; not saved; CKP-8)

ListUI
  filter          ∈ {All} ∪ 0..6
  page            ≥ 1
  confirm         ∈ {None} ∪ Result             (the delete dialog)

Settings
  color_starting, color_clash, color_highlight, color_completed ∈ Palette
  micro_line      ∈ {None, gray90, gray80, gray70, gray60, gray55, gray50, gray45, gray40}   (Cell colors panel; default gray90; COL-7)
  last_mode       ∈ 0..6
  highlight_on    : bool                       (Settings panel; default true)
  auto_hide_on    : bool                       (Settings panel; default true)
  completed_color_on : bool                    (Settings panel, "Color completed groups"; default true)

Update
  current         : text                       (the version of the app: v<version>, pre-<sha>, dev or dev/<name>)
  state           ∈ {Off, Unknown, CheckFailed, UpToDate, Available(v), Installing(v), Failed(v, at), Installed { patch }}
  checking        : bool                       (a request for version.json is in progress)
  up_to_date_shown : bool                      (the row shows "Sudoku is up to date"; UPD-9)
  dialog          : bool                       (the dialog "Restart the interface?" shows; UPD-10)
                                               (code: these three fields are in the variants of
                                                UpdateState: CheckFailed { checking },
                                                UpToDate(CheckButton | Checking | UpToDateText) and
                                                Installed(Replaced { dialog }); refer to src/update.rs)

Palette = {Gray, White, Yellow, Blue, Red, Green, Cyan, Magenta, Orange}

Result
  id, finished_at
  mode            ∈ 0..6
  givens          ∈ 0..81
  seconds         ≥ 0
  seed, generator
```
<!-- invariants:state-model:end -->

Terms:

<!-- invariants:terms:start -->
- **peers(r, c)**: the 20 other cells in row r, in column c and in the 3×3 box of (r, c).
- **filled(r, c)**: `value[r][c] ≠ 0`.
- **entry(r, c)**: the cell is filled and is not a starting number ("your number").
- **clashing(r, c)**: the cell is filled and a peer has the same value.
- **group**: the nine cells of a row, of a column or of a 3×3 box.
- **completed group**: a group that agrees with COL-8. It is not the same as a complete game
  (`completed`).
- **shows_completed(r, c)**: (r, c) is in a completed group, `completed_color_on`,
  `color_completed ≠ White` and `¬completed`.
- **can_undo / can_redo**: the stack is not empty and the game is not complete.
- **same_board(a, b)**: a and b have the same `value` and the same `marks` in all cells. This includes the
  hidden marks. It does not include the clock.
- **current(k)**: `same_board(k, game)` for a checkpoint k.
- **can_save**: `¬completed`, `|checkpoints| < 5`, and no checkpoint is `current`.
- **Restore**: the change that replaces all of `value` and `marks` with those of a checkpoint (CKP-2).
- **refused tap**: a pen tap on the micro-cell of digit d of an empty cell, when `auto_hide_on` and a
  peer has the number d (MRK-7).
- **hand touch**: a touch that starts less than 150 ms after a pen lift (INP-20).
- **the player writes**: `pending_ink ≠ None`, or the writing timer runs (INP-16).
- **full reversal**: the same strokes with the last stroke first, and with each stroke from its end to
  its start. Ink with only the stroke order or only the stroke directions changed is not a full
  reversal.
- **example copy**: one item of the recognizer: one sample or shape of a source, with one slant,
  rotation and width for a digit, and with no change for "not 1–9". The sources are the database of 60
  writers, the typeface shapes and the handwriting of the owner.
<!-- invariants:terms:end -->

## 1. Puzzle and solution

| ID | Rule | Source |
|---|---|---|
| PUZ-1 | `solution` is a complete, correct sudoku. Each row, column and box has the digits 1–9 one time. | R§14.5 |
| PUZ-2 | The starting numbers have **only one** solution. That solution is `solution`. | R§14.5 |
| PUZ-3 | For each starting number, `value = solution`. Thus two starting numbers never clash. | R§14.5 |
| PUZ-4 | `given` does not change after the generator makes the puzzle. It changes only when a new puzzle replaces the game. | R§4.1 |
| PUZ-5 | `(seed, mode)` and the version of the generator set the full game. The same values always give the same starting numbers and the same solution. A saved game whose starting numbers or solution are not `generate_version(generator, seed, mode)` is not correct: the app does not use it (PER-5). A new game has the current version (2). Version 1 has only the modes 0..4. A game file with no version is version 1. | R§14.1, R§14.5 (confirmed 2026-10-25; versions: owner decision 2026-10-25) |
| PUZ-6 | The generator sets the level (the most difficult group of techniques) of each mode. The human-style solver, with all techniques, completes each puzzle, and the most difficult technique that it needs is of the level of the mode. Exception: if no attempt of the 400 needs the level of the mode, the puzzle is the attempt with the most difficult grade, and its level can be easier (R§14.5). Easy has 44 starting numbers. No other part of the app uses a fixed level or a fixed quantity of starting numbers. | R§11, R§14.5 (owner decisions 2026-10-25 and 2026-11-02) |

## 2. Cells

| ID | Rule | Source |
|---|---|---|
| BRD-1 | Each cell is always one of these types: starting number (`given`), your number (`entry`), empty with no marks, empty with marks. | R§4.1 |
| BRD-2 | `given ⇒ filled`. The value of a starting number does not change. | R§4.1 |
| BRD-3 | The value of an entry is 1–9. Only the eraser, undo, redo or a Restore can remove or change it. Writing does not replace it. | R§4.1, R§5.1, R§9.3, owner approval 2026-11-06 |
| BRD-4 | A value changes from 0 to a digit only by a number from 1 to 9 that the app reads, by redo, by undo of an erasure, by a Restore, by undo of a Restore, or by the generator. | R§5.1, R§7, R§9.3, owner approval 2026-11-06 |
| BRD-5 | An entry can be incorrect. `value ≠ solution` is a correct state. The app gives no cell a color because of `value ≠ solution`: an incorrect entry gets a color only by the rules that also apply to a correct entry (BRD-7). | R§6.3, owner decision 2026-11-05 |
| BRD-6 | Filled cells do not show the micro-cell grid or pencil marks. Empty cells show the micro-cell grid in the gray of `micro_line` (COL-7), or no grid if it is None. The only exception is an empty cell with a "?" (QST-1): it does not show the grid while the "?" shows. | R§4.1, R§5.1, R§10 |
| BRD-7 | The background of a filled cell is the **first** of these: the clash color if clashing; the highlight color if `value = highlight`; the completed color if `shows_completed`; the starting color if given; white. Empty cells are white. Thus a cell with a "?" (QST-1) has no completed color. | R§4.2, owner decision 2026-11-05 |
| BRD-8 | Starting digits are black. Entry digits are `#1E1E1E`. | R§4.1, R§15, owner decision 2026-11-05 |

## 3. Pencil marks

| ID | Rule | Source |
|---|---|---|
| MRK-1 | `marks[r][c] ⊆ {1..9}`. Each mark is at its keypad position (1 at the top left, 9 at the bottom right). | R§3.1 |
| MRK-2 | Marks **show only when the cell is empty**. A filled cell can have marks, but they are hidden. | R§4.1 |
| MRK-3 | Starting numbers never have marks. You can add marks only to empty cells, and starting numbers are never empty. | R§4.1 |
| MRK-4 | **Automatic hiding:** if `auto_hide_on`, an empty cell shows the marks `marks[r][c]` minus the digits of the numbers in its peers. A filled peer keeps its hidden marks with no change. | R§6.1 |
| MRK-5 | Marks change only in these ways: a pen tap adds or removes one mark in an empty cell; a Restore sets all marks to those of the checkpoint; undo and redo of these changes. A new number, the eraser and the switches never change marks. Exception: undo and redo of a number of an earlier version that has a `removed` list (R§6.1). | R§5.1, R§6.1, R§7, R§9.3, owner approval 2026-11-06 |
| MRK-6 | When a number goes (the eraser, undo or redo), the hidden marks of that cell show again with no change. A Restore and its undo and redo are different: the cell has the marks of the board that the change sets (CKP-2). | R§4.1, R§6.1, R§9.3, owner approval 2026-11-06 |
| MRK-7 | If `auto_hide_on`, a pen tap on micro-cell d of an empty cell does not change the game when a peer has the number d. This is also true when the cell has the hidden mark d. This tap is a refused tap, and it shows the x (MRK-10). If `¬auto_hide_on`, the tap adds or removes the mark d, and no x shows. A mark does not have to be a possible digit. | R§6.1 (owner decisions 2026-10-31 and 2026-11-04) |
| MRK-8 | When a number d goes (the eraser, undo or redo), the marks d of its peers show again, if no other peer of those cells has the number d. After a Restore and its undo and redo, MRK-4 gives the visible marks from the board that the change sets. | R§6.1 (owner decision 2026-10-31), R§9.3, owner approval 2026-11-06 |
| MRK-9 | If `¬auto_hide_on`, each empty cell shows all of `marks[r][c]`. A new number never has removed marks, and its undo and redo change only that cell. | R§6.1, R§9.1 |

## 4. Clashes

| ID | Rule | Source |
|---|---|---|
| CLS-1 | The set of clashing cells is always `{(r, c) : clashing(r, c)}`. The app calculates it from `value` after each change: a new number, an erasure, a Restore, undo, redo (also of a Restore) and a new game. The app does not keep it separately from the board. | R§6.3, R§9.3, owner approval 2026-11-06 |
| CLS-2 | Clashes go in two directions. If (r, c) clashes with a peer, that peer also clashes. Both cells get the clash color. | R§6.3 |
| CLS-3 | Two starting numbers never clash (PUZ-3). Thus each clash includes one or more entries. | R§6.3, R§14.5 |
| CLS-4 | The app never compares the board with `solution` for the player. An incorrect entry with no clash looks the same as a correct entry. This is also true in a completed group: the completed color does not tell that a number is correct or incorrect (COL-9). | R§6.3, owner decision 2026-11-05 |

## 5. Overlays: highlight, "?" and x

| ID | Rule | Source |
|---|---|---|
| HLT-1 | `highlight ≠ None ⇒` the game is not complete. | R§6.2, R§8.3 |
| HLT-2 | `highlight` gets a value only from a pen tap on a number (HLT-3). Only these remove a highlight or move it: (1) a pen tap on a number, at the pen lift, by the value of `highlight` at the press (HLT-3); (2) a pen tap on an empty cell, at the pen lift, in the same redraw as the mark or the x; (3) while `panel = None`, a pen stroke that starts on a filled cell, at the pen lift; (4) a read of waiting ink, by any cause, in the same redraw as the number or the "?"; (5) a new game, a restart of the app (PER-2) or a completed puzzle (HLT-1); (6) a change of screen to Manual or Statistics (HLT-4); (7) `dialog` becomes true (UPD-10): the highlight does not come back after "Later"; (8) turning off `highlight_on` (HLT-6). All other input keeps `highlight` (HLT-7). | R§3.2, R§6.2, owner decision 2026-11-07 |
| HLT-3 | A tap on a number compares its value with `highlight` as it was at the press. At the pen lift, a tap on a number with a value equal to that `highlight` sets `highlight = None`. If `highlight_on`, a tap on a number with a different value sets `highlight` to that value (HLT-6). | R§6.2, owner decision 2026-11-07 |
| HLT-4 | `highlight ≠ None ⇒ screen = Game`. A highlight can show while the menu or a panel is open. A change of screen to Manual or Statistics sets `highlight = None`, and the highlight does not come back on return. While the menu is open and no panel is open, a pen tap on a number operates as in HLT-3. | R§9, R§6.2, owner decision 2026-11-07 |
| HLT-5 | A highlight does not change `value`, `marks`, the undo and redo stacks, or `completed`. | R§7 |
| HLT-6 | `¬highlight_on ⇒ highlight = None`. Turning off `highlight_on` removes a highlight in that redraw, and the panel stays open. A tap on a number then makes no highlight (it still removes a "?" and an x, QST-4, MRK-15). | R§6.2, R§9.1, owner decision 2026-11-07 |
| HLT-7 | These do not change `highlight`: a finger or a hand on the board or anywhere else, also a second finger (INP-15); the menu, the panels and their options, also a pen press on the board while a panel is open, which only closes the panel, also if the pen then moves (NAV-11), and a tap that only closes a panel (NAV-12); Undo, Redo and Restore; the eraser, also on waiting ink in its own cell; a pen press off the board on no control when no ink waits; a turn of the tablet (NAV-13); a suspension. If one of these makes the app read waiting ink, the read removes the highlight (HLT-2, item 4). | R§6.2, R§9, owner decision 2026-11-07 |
| HLT-8 | `highlight` can be a digit that no cell has now. If Undo, Redo or Restore removes the last number with the digit `highlight`, `highlight` does not change, and the highlight shows again when that digit comes back. A tap on that digit then sets `highlight = None` (HLT-3). | R§6.2, owner decision 2026-11-07 |
| HLT-9 | A pen press does not change `highlight`, unless that press makes the app read waiting ink (INP-12). Thus `highlight ≠ None` and `pending_ink ≠ None` can be true at the same time. The removal or the move of a highlight goes to the screen in one redraw with the result of the action: a tap at the pen lift, and writing when the app reads the ink. When the pen area is in use, two updates occur only in these two cases: a pen press on a control while ink waits (the number at the press, the action at the release), and a press on a control that removes an x or a "?" (the removal at the press, the action at the release). After a pen stroke that starts on a filled cell, `highlight = None` from the pen lift, but the pixels can stay until the writing timer ends (up to 1 s). A tap on that number in this time makes a highlight (HLT-3). A tap on a number while ink waits in a different cell reads the ink at the press (INP-12), and that read removes a highlight v (HLT-2, item 4). At the lift, HLT-3 uses the value at the press: a tap on v gives `highlight = None`, and a tap on a different digit w gives `highlight = w`. When the pen area is in use, the read and the result of the lift go in one redraw, and a "?" from that read can show with highlight w. When the ink patch is not active, redraws do not wait for the writing, thus this tap gives two updates: the read at the press and the highlight at the lift. | R§3.2, R§6.2, R§14.4, owner decision 2026-11-07 |
| QST-1 | `question ≠ None ⇒` the cell of the "?" is empty. The "?" shows for approximately one second. Then `question = None`. While the "?" shows, the marks of the cell are hidden. After the "?", the cell is the same as before. | R§5.1 |
| QST-2 | The "?" does not change `value`, `marks` or the undo and redo stacks. | R§5.1, R§7 |
| QST-3 | `question ≠ None ⇒` the game is not complete. | R§3.2 |
| QST-4 | Each press of the pen or a finger removes the "?" immediately. The exceptions are a hand touch (INP-20) and a first press that the app ignores after "< Back" (NAV-16). Thus a new number in that cell never shows under the "?" (QST-1). | R§5.1 (confirmed 2026-10-25), owner decision 2026-11-07 |
| MRK-10 | Only a refused tap sets `refused`. At the pen lift, not at the pen down, it sets `refused = (cell, d, shown_at)`. A finger, the eraser and writing never set `refused`. These pen touches also do not set it: a tap on a filled cell, a tap when `completed`, a tap while `panel ≠ None` (NAV-11: the tap only closes the panel), and a short touch in the cell of waiting ink (INP-11: this is writing). When `menu = Open` and `panel = None`, a refused tap sets `refused` as usual. | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-11 | A refused tap and the x do not change `value`, `marks`, the undo and redo stacks, the clock or `completed`. A refused tap does not empty `redo`. They do not cause a save. | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-12 | `refused = (cell, d, shown_at) ⇒` all of these are true: `screen = Game`, `panel = None`, the game is not complete, the update dialog does not show, `cell` is empty, `auto_hide_on`, a peer of `cell` has the number d, `highlight = None` and `pending_ink = None`. If `question ≠ None`, its cell is not `cell`. Thus an open panel never shows an x, and a mark never shows under an x. | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-13 | The x shows only in its micro-cell, in all layouts. It has two straight lines with round ends, from -10 px to +10 px on the two axes around its center. The line width is 3 px and the color is gray15. Its center is the anchor point of the pencil mark of d: 24, 74 or 122 px from the cell origin on each axis, at the keypad position of d (MRK-1). The other visible marks and the micro-cell grid of the cell do not change (with `micro_line = None`, the x shows with no grid). If nothing else changes, the pixels of the cell after the x are the same as before the x. | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-14 | The x shows for the same time as the "?" (QST-1): the two use one value, 1000 ms. Then `refused = None`, with no other input: the main loop wakes at the end of this time. A "?" and an x can show at the same time in different cells, each with its own time. At most one x shows. | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-15 | Each press of the pen or a finger, also a second finger (INP-15), sets `refused = None` before the app does the action of that press. The exceptions are a hand touch (INP-20) and a first press that the app ignores after "< Back" (NAV-16). Thus a new refused tap removes the x at the press and shows it again at the lift, and the time starts again. The release of a control that does an action also sets `refused = None`, and it does not remove the highlight (HLT-7). Thus an undo that removes the number of the peer never leaves an x. Completion, a change of screen, a new game and the update dialog also set `refused = None`. While the player writes, screen redraws wait (R§14.4): the pixels of the x can stay under the ink until the redraw, but `refused = None` from the press. | R§5.1, R§6.1, owner decisions 2026-11-04, 2026-11-05 and 2026-11-07 |
| MRK-16 | A turn of the tablet does not change `refused`: the x stays on the same cell and digit for the rest of its time. A tap in progress at the turn sets no x, because the turn stops the pen touch (NAV-13). | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-17 | The app does not save `refused`. The app always opens with `refused = None` (PER-2). | R§5.1, R§6.1, owner decision 2026-11-04 |
| MRK-18 | `refused` comes only from the numbers in the peers, not from `solution`. A peer with an incorrect number of the player also causes an x. | R§5.1, R§6.1, owner decision 2026-11-04 |

## 6. Input

| ID | Rule | Source |
|---|---|---|
| INP-1 | Finger touches on the board do not change the game state. The only exceptions are the removal of the "?" (QST-4) and of the x (MRK-15). A finger does not remove a highlight (HLT-7). A finger can operate all items that are not on the board: the menu buttons, the panels, the dialogs and the screen buttons. The exception is the close button, which a finger does not operate (NAV-6). A hand touch does none of these (INP-20). In the manual and the games list, a horizontal finger swipe can change the page. | R§5.1, R§12, R§13.2, owner decision 2026-11-07 |
| INP-2 | A pen touch on the board is a **tap** only if all of its movement stays in one micro-cell. All other pen touches on the board are writing. Each pen touch on the board is one of these: a tap, writing, or the eraser. Each pen press on an empty cell starts the pen area at the pen down (INP-24), also a tap. | R§5.1, R§14.4 (owner decisions 2026-11-02 and 2026-11-07) |
| INP-3 | A tap on an empty cell adds or removes one mark: the digit of the micro-cell that you tapped. If `auto_hide_on` and a peer has that digit, the tap does not change the game, and the x shows (MRK-7, MRK-10). | R§4.1, R§5.1, R§6.1 |
| INP-4 | While no panel is open, a tap on a filled cell does not change that cell. It changes only `highlight` (HLT-3, HLT-6), the "?" (QST-4) and the x (MRK-15: the tap never shows an x), and it makes the app read ink that waits in another cell (INP-12). | R§5.1, R§6.2 |
| INP-5 | `pending_ink ≠ None ⇒` its cell is empty and the game is not complete. The app collects the ink until the recognition pause ends. The recognition pause is 1 s (INP-23). It starts at the pen lift, and a new stroke in the same cell stops it. At the end of the pause, the app reads all the ink as one item and sets `pending_ink = None`. | R§5.1, owner decision 2026-11-07 |
| INP-6 | The app reads the ink as one of these two results: a number from 1 to 9 (the app adds it), or "not 1–9" (scribble, letter, 0, 10 or more). For "not 1–9", the app shows the "?". There is no other result. | R§5.1 |
| INP-7 | A pen stroke that starts on a filled cell (starting number or entry) does not change `value`, `marks` or the undo and redo stacks, and the app does not read it (INP-22). It removes a highlight at the pen lift (HLT-2). | R§5.1, owner decision 2026-11-07 |
| INP-8 | The eraser changes the board only on an entry. It makes the cell empty and shows its hidden marks again. On starting numbers and empty cells, it makes no change. The eraser on the cell of the waiting ink removes the ink. The app does not read that ink, thus the board and the undo history do not change. | R§5.1 (confirmed 2026-10-25) |
| INP-9 | When `completed` is true, pen input does not change `value`, `marks`, `highlight`, `refused` or the stacks. | R§8.3 |
| INP-10 | Ink never stays on the board. After the app reads it, removes it or shows the "?", the board shows only typed numbers, marks, the grid, the "?" and the x (MRK-10). | R§5.1 |
| INP-11 | While ink waits in a cell, each pen touch that starts in the same cell is writing. This is also true if the touch stays in one micro-cell. Thus a short stroke of a number (for example the bar of a 7) does not become a pencil mark. | R§5.1 (confirmed 2026-10-25) |
| INP-12 | Writing belongs to the cell where the pen touch started. A pen touch that starts in a different cell, or a pen press off the board (on a control or on no control), makes the app read the waiting ink immediately, and it stops the writing timer (INP-16). A finger reads it only at the release of a control (INP-21). | R§5.1 (confirmed 2026-10-25), owner decision 2026-11-07 |
| INP-13 | The eraser removes all of your numbers that it touches in one stroke. Each removal is one change in the undo history. | R§5.1, UND-1 (confirmed 2026-10-25) |
| INP-14 | A finger swipe is a horizontal movement of 120 px or more, with more horizontal than vertical movement. | R§5.1, R§12, R§13.2 (confirmed 2026-10-25) |
| INP-15 | A finger press on an item that is not on the board (INP-1: menu buttons, panels, dialogs, screen buttons) operates it while the app is open. The exceptions are the close button (NAV-6), a hand touch (INP-20) and a dialog that shows: then only its two buttons operate. If the app does not get the release of a touch, that touch does not stop other presses: the app forgets it when a new press with the same touch id starts, or when a new touch starts and that touch has had no event for 0.15 s. A second finger while the first finger moves or presses (an event less than 0.15 s before) operates no item, but it removes the "?" and the x (QST-4, MRK-15). It does not remove a highlight (HLT-7). | R§9, R§5.1, owner decision 2026-11-07 |
| INP-16 | The writing timer (a redraw waits, R§14.4) runs only on the game screen of a game that is not complete. Each stroke on the board starts it again (1 s, equal to the recognition pause of INP-5). A pen press that does not write stops it. A finger press does not stop it (INP-21). | R§5.1, R§14.4 (owner decisions 2026-11-02 and 2026-11-07) |
| INP-17 | For all classes (the digits and "not 1–9") and all sources, the recognizer stores each example copy exactly two times: one time as written and one time as its full reversal. No example copy has only one direction, and no example copy has two items in one direction. Thus a typeface shape has no separate reversed item. A sample of the database of 60 writers has 2 stored items for "not 1–9" and 18 stored items for a digit. | R§5.1 (owner decision 2026-11-06) |
| INP-18 | The app does not reverse the ink of the player. For one set of strokes, it calculates the features one time, as written, and it compares them with all stored items (INP-17). | R§5.1 (owner decision 2026-11-06) |
| INP-19 | When the examples of writers 1 to 40 read the 3360 inks of writers 41 to 60, each ink and its full reversal give the same result. The full reversals of these inks agree with the target of R§5.1 (96 % or more of the digits correct, 1.2 % or less wrong). This rule is for this test set only: it does not apply to all ink, and it does not apply to partly reversed ink. | R§5.1 (owner decision 2026-11-06) |
| INP-20 | A hand touch (a touch that starts less than 150 ms after a pen lift) has no effect at all: it does not operate a control, it does not remove a "?" or an x, and its move and its release also have no effect. A touch that starts 150 ms or more after a pen lift operates as usual, also if it comes from the hand. | R§5.1, R§9, owner decision 2026-11-07 |
| INP-21 | A finger press never reads waiting ink and never stops the writing timer, on the board and off the board. A control that a finger operates reads the waiting ink and stops the writing timer at its release, on the control. A release off the control does nothing. | R§5.1, R§9, R§14.4, owner decision 2026-11-07 |
| INP-22 | A pen touch that starts on a filled cell shows no ink and starts no pen area. This includes a tap on a number and a stroke that starts on a number. A stroke that starts on an empty cell and crosses a filled cell shows ink. The corpus build logs the strokes that do not show. | R§5.1, R§14.4, owner decision 2026-11-07 |
| INP-23 | The recognition pause has one value in the source, 1000 ms. The AppLoad manifest (`external.manifest.json`) does not set it, and no environment variable changes it. | R§5.1, R§14.4, owner decision 2026-11-07 |
| INP-24 | Each pen press on an empty cell starts the pen area at the pen down, also a tap and also the first stroke of a session, and a press on a different empty cell moves it with no stop between the cells. The area never covers a filled cell (INP-22). When the pen lifts, the area stays on its cell. It changes only when a press on a different empty cell moves it, when its cell becomes filled, when a redraw needs the UI waveform in its cell (a number, a "?", other micro-cell lines, or a panel, a dialog or a different screen over the cell: then the area stops before that redraw), or at a turn of the tablet. A tap: the state changes at the pen lift, and its mark or x goes to the screen at the pen lift inside the area, with the pen waveform, only in its micro-cell, together with the removal of its ink. A tap starts no writing timer (INP-16). | R§5.1, R§14.4, owner decision 2026-11-07 |

## 7. Undo and redo

| ID | Rule | Source |
|---|---|---|
| UND-1 | Each change to the board is one of these: add or remove a mark; add a number; erase a number; a Restore (one item for all cells). Each change adds one item to `undo` and empties `redo`. | R§7, R§9.3, owner approval 2026-11-06 |
| UND-2 | No other action adds to `undo`. Highlights, the "?", the x, color changes, switch changes, a group that gets or loses the completed color, panels, screens, the save and the delete of a checkpoint, and a tap on "Current" never add to it. | R§7, owner decision 2026-11-05, R§9.3, owner approval 2026-11-06 |
| UND-3 | Undo does the exact opposite of the top change and moves the change to `redo`. Redo does the change again and moves it back to `undo`. An undo and then a redo give the same board as before. A redo and then an undo also give the same board. | R§7 |
| UND-4 | Undo of a number shows the hidden marks of that cell again, and the marks of the peers that the number hid (MRK-8). For a number of an earlier version with a `removed` list, undo puts back those marks. | R§6.1 |
| UND-5 | After each undo or redo, CLS-1 is true. The app calculates the clashes again. | R§6.3 |
| UND-6 | A new game sets `undo = redo = ∅` and `checkpoints = ∅`. | R§7, R§9.3, owner approval 2026-11-06 |
| UND-7 | The undo button is black only if `can_undo`. The redo button is black only if `can_redo`. In all other conditions, the button is gray and has no effect. | R§7 |
| UND-8 | `completed ⇒` the two buttons are gray and have no effect. This is also true when the stacks are not empty. | R§7, R§8.3 |
| UND-9 | Undo, redo and a Restore do not change `given`, `solution`, `seed`, `mode` or `elapsed`. | R§7, R§9.3, owner approval 2026-11-06 |

## 8. Completion and the lock

| ID | Rule | Source |
|---|---|---|
| CMP-1 | `completed ⇔` all cells are filled and no cell clashes. The app sets it to true immediately when this becomes true. Only a new game sets it to false. | R§8.1, R§8.3 |
| CMP-2 | Each row, column and box of a complete board has the digits 1–9 one time. Thus a complete board is the same as `solution` (PUZ-2). | R§8.3, R§14.5 |
| CMP-3 | `completed ⇒` the board, the marks, the stacks and `checkpoints` do not change (INP-9, UND-8, CKP-9), and `panel ≠ Checkpoints`, `highlight = None`, `question = None`, `refused = None`, `pending_ink = None`. | R§8.3, R§9.3, owner decision 2026-11-06 |
| CMP-4 | `completed ⇒` the text "Completed in" and `elapsed` replace the meter below the board. The format is `m:ss`, and `h:mm:ss` after one hour. `¬completed ⇒` the meter shows `mode`. The player cannot change the meter. | R§3.1, R§8.3 |
| CMP-5 | Completion adds **one** row to `results`. The row has the `mode`, the quantity of starting numbers, the final `elapsed`, the `seed` and the `generator` version of this game. The app never adds a row for a game that is not complete. | R§8.3, R§14.1 |
| CMP-6 | Only Start new game ends the `completed` state. Help, Statistics, colors, settings and the menu continue to operate. The Checkpoints panel is not available (CKP-9). | R§8.3, R§9.3, owner decision 2026-11-06 |

## 9. Clock

| ID | Rule | Source |
|---|---|---|
| CLK-1 | `elapsed` increases only when the app is open (on all screens) and the game is not complete. | R§8.4 |
| CLK-2 | `elapsed` never decreases. The only exception is a new game, which sets it to 0. | R§8.4 |
| CLK-3 | `completed ⇒ elapsed` does not change. It is the final time and is equal to `seconds` in the saved result. | R§8.4, R§14.1 |
| CLK-4 | The app does not show the clock during the game. The game screen shows the time only in "Completed in …". | R§8.4 |
| CLK-5 | The app does not count the time when the app is closed or suspended. When the app opens again, the clock continues from the saved `elapsed`. | R§8.4, R§14.3 |
| CLK-6 | If the app gets no events and no clock ticks for more than 5 seconds, the clock does not count that time (the app was suspended). The app ticks at least one time each second while it operates. | R§8.4 (confirmed 2026-10-25) |

## 10. Screens, menu and panels

| ID | Rule | Source |
|---|---|---|
| NAV-1 | The app shows one screen at a time: Game, Manual, Statistics, GamesList, Licenses or LicenseText. The app starts on Game, with `menu = Open` (PER-2). | R§2 |
| NAV-2 | Only the open menu opens Manual and Statistics. Only Statistics opens GamesList. Only the Settings panel opens Licenses, and only Licenses opens LicenseText. "< Back" goes from Manual to Game, from Statistics to Game, from GamesList to Statistics, from Licenses to Game and from LicenseText to Licenses. | R§2, R§9.4 |
| NAV-3 | When Manual, Statistics or Licenses shows Game again, `menu = Open` and `panel = None`. | R§9, R§12, R§13.1 (owner decision 2026-11-02) |
| NAV-4 | `panel ≠ None ⇒ menu = Open`. Only one panel can be open at a time. | R§9 |
| NAV-5 | `screen ≠ Game ⇒ menu = Closed, panel = None`. The menu is a part of the game screen. When the game screen shows again, the menu opens (NAV-3). | R§9 |
| NAV-6 | The close button shows only when `screen = Game` and `menu = Open`. When `panel = None`, a tap of the pen or of its eraser end on it closes the app after the app saves the game (PER-1). A finger never closes the app: a finger press on it, and its release, have no action, and the press only removes the "?" and the x (QST-4, MRK-15). When `panel ≠ None`, a tap of the pen or of a finger on it only closes the panel (NAV-12). | R§3.1, R§9 (owner decision 2026-11-02, owner approval 2026-11-06) |
| NAV-7 | Icon of the menu button: the dot is at the bottom left only when `menu = Closed`. The dot is at the top left only when `menu = Open`. For a right-handed player the icon is a mirror image (the dot is at the right; NAV-15). | R§3.1 |
| NAV-8 | The button of a panel is white on black only when that panel is open. The other buttons have their usual appearance. | R§9 |
| NAV-9 | After Start new game, `menu = Open` and `panel = None`. | R§9, R§11 (owner decision 2026-11-02) |
| NAV-10 | The closed menu never covers the board. The open toolbar (portrait x ≥ 1508, landscape x ≥ 2048) never covers the board (portrait x ≤ 1471, landscape x ≤ 1741). An open panel can cover a part of the board. For a right-handed player, the toolbar is at the left edge (x 0–112) and the board does not change (NAV-15). | R§3.1, R§3.3 |
| NAV-15 | When the device is right-handed, the game screen shows the toolbar, the menu button, the close button and the panels as a mirror image of the left-handed screen: the toolbar is at the left edge (x 0–112), the close button is at the top right, and the panels touch the toolbar at x 112. The icons other than the menu button and the contents of the panels are the same. The hit areas move with the buttons. | R§3.1 (owner request 2026-10-26) |
| NAV-13 | A turn of the tablet does not change the game, the settings or the screen. It removes the pending ink and the touches in progress, and it changes only the page of the games list: the list shows the page with the first game of the page before the turn (R§3.3). | R§3.3 (confirmed 2026-10-25) |
| NAV-14 | The Settings panel opens from the gear (top slot 2). It is 400 px wide (x 1108–1507 for a left-handed player in portrait). Rows 1 to 3 have the switches `highlight_on`, `auto_hide_on` and `completed_color_on`, in this order. A tap on one of these rows changes only its switch, immediately; the panel stays open. The only exception: turning off `highlight_on` also removes a highlight (HLT-6). A tap on the version row (row 4) changes nothing, and the panel stays open. A tap on row 5 opens Licenses (LIC-1). A change of a switch does not change the game or the undo and redo stacks, and it does not change the changes that are already in the stacks. The switches also operate when `completed` (CMP-6). | R§9.1 (version row: owner decision 2026-10-26; third switch and width: owner decision 2026-11-05; highlight: owner decision 2026-11-07) |
| NAV-11 | When `panel ≠ None`, a tap outside the panel only sets `panel = None`. It does not change the game, also when the tap is on the board. A pen press on the board while a panel is open also only closes the panel, also if the pen then moves: it is not a tap or a stroke on the board, and it keeps `highlight` (HLT-7). The only exception is the guard after a delete (CKP-8). When `menu = Open` and `panel = None`, the board operates as usual. | R§9, R§9.3, owner decision 2026-11-06 and 2026-11-07 |
| NAV-12 | When `panel ≠ None`, a tap on a toolbar button that is not the paint bucket, the gear, the bookmark or the + only closes the panel. This includes the menu button, the close button, undo and redo: with an open panel, the first tap on undo only closes the panel, and a second tap does the undo. A tap on the button of a different panel opens that panel. The only exception is the gray bookmark: a tap on it changes nothing, and an open panel stays open (CKP-9). | R§9 (confirmed 2026-10-25), R§9.3, owner decision 2026-11-06 |
| NAV-16 | For 0.5 s after "< Back" from Manual, Statistics or Licenses shows Game, a press on a toolbar button (also the menu button and the bookmark in top slot 5) or on the close button changes nothing: `menu`, `panel`, `screen` and the game do not change, and the app does not close. A first press that the app ignores in this time does not remove the x or the "?" (the exception to MRK-15 and QST-4). A second finger on one of these items, while another finger stays on the screen, operates no item, but it removes the x and the "?" (INP-15). A press on the board operates as usual. This is true for the left-handed and the right-handed layout. | R§9, R§12, R§13.1 (owner decisions 2026-11-02, 2026-11-04 and 2026-11-05), R§9.3, review finding 2026-11-06 |
| NAV-17 | The Cell colors panel is 800 px wide and covers y 112 to 1192, with the two ends included (1081 rows: three rows of 360 px and the last border row). Its x range, also with the two ends included, is 708–1507 for a left-handed player in portrait, 1248–2047 for a left-handed player in landscape, and 112–911 for a right-handed player in the two orientations. It has two columns of 400 px, in the same order for the two hands. The left column has, from the top, Starting numbers, Clashing numbers and Highlighted numbers. The right column has, from the top, Completed groups, Micro-cell lines and empty space. In all four layouts, the panel does not cover a toolbar button, the close button, the meter or "Completed in". A tap on the empty space or on a title changes nothing, and the panel stays open. | R§3.1, R§10, owner decision 2026-11-05 |
| NAV-18 | The Checkpoints panel opens from the bookmark (top slot 5, y 560–671). A second tap on the bookmark closes it. It is 600 px wide. Its x range, with the two ends included, is 908–1507 for a left-handed player in portrait, 1448–2047 for a left-handed player in landscape, and 112–711 for a right-handed player in the two orientations. It starts at y 560 in all layouts. It ends at y 864 with no checkpoint, and at y `672 + 180 × n` with n checkpoints, n ≥ 1 (y 1572 with 5). The contents have the same order in all layouts. In all four layouts, the panel does not cover a toolbar button, the close button, the meter or "Completed in". `panel = Checkpoints ⇒ pending_ink = None` and no stroke is live. The pen, the eraser and a finger operate the panel in the same way. A tap in the panel on no control changes nothing, and the panel stays open. | R§3.1, R§3.3, R§9.3, owner approval 2026-11-06 |

### 10.1 Checkpoints panel

The x values in this table are for a left-handed player in portrait. In the other layouts, each x value keeps the same distance from the left edge of the panel (NAV-18). For row i of the list, `y0 = 672 + 180 × i`.

| ID | Rule | Source |
|---|---|---|
| CKP-1 | The row "Save checkpoint" (y 560–671; the full row is the tap area) is black exactly when `can_save`, and else gray 70. When `can_save`, a tap adds one checkpoint at the start of `checkpoints`, with `value`, `marks` and `elapsed` of the game. It does not change `value`, `marks`, the stacks or `elapsed`. The app saves the game file. When `¬can_save`, a tap changes nothing. | R§9.3, owner approval 2026-11-06 |
| CKP-2 | The button of a row shows "Current" exactly when the checkpoint is `current`, and else "Restore". A tap on "Restore" sets `value` and `marks` to those of the checkpoint, adds one Restore item to `undo` and empties `redo`. Undo of this item gives the board before it. Redo gives the board of the checkpoint, also after the delete of the checkpoint (CKP-3). A Restore does not change `elapsed` or `checkpoints`, and `panel` stays `Checkpoints`. A tap on "Current" changes nothing and does not empty `redo`. A checkpoint that is different only in hidden marks shows "Restore", and a tap adds one undo item with no visible change. | R§9.3, owner approval 2026-11-06 |
| CKP-3 | A tap on the bin of a row removes that checkpoint immediately, with no dialog and no undo. It does not change `value`, `marks`, the stacks or `elapsed`. The app saves the game file. A Restore item in the stacks does not change, and its undo and redo give the same boards after the delete of its checkpoint. | R§9.3, owner approval 2026-11-06 |
| CKP-4 | `checkpoints` has 0 to 5 items, newest first. No two items have `same_board`. The `elapsed` of each item is at most the `elapsed` of the next newer item, and at most the `elapsed` of the game. | R§9.3, review finding 2026-11-06 |
| CKP-5 | When the app opens a saved game, these must be true for each checkpoint and for each of the two boards of each Restore item: each value is 0 to 9; a given cell has its value from `solution` and has no marks; the marks of all other cells are in 1 to 9; the board is not solved (all cells filled and no clash). For each Restore item, `before` is the board at that point of the history, and `before` is not `same_board` as `after`. A checkpoint on a complete game is correct. A game file that breaks this rule or CKP-4 is incorrect saved data, and the app makes a new game (PER-5). A game file of an earlier version has no checkpoints: `checkpoints = ∅`. | R§9.3, review finding 2026-11-06, PER-5 |
| CKP-6 | Each change of `checkpoints` stops all other pen and finger gestures that are in progress. Thus a release never operates a different checkpoint. | R§9.3, review finding 2026-11-06 |
| CKP-7 | With `checkpoints = ∅`, the panel shows "No checkpoints" and "Save the board before a difficult guess.", and no row. Each row shows a small board, `<clock> played` (the `elapsed` of the checkpoint in the format of CMP-4), `<n> of 81 cells` (n counts all filled cells of the checkpoint, also the starting numbers), the button and the bin. A row has no title and no name. On the small board, the background of a filled cell is the first of these, from the values of the checkpoint: the clash color if clashing; the completed color with the switch and color rules of `shows_completed`; the starting color if given; white. It never has the highlight color. The digit has the color of BRD-8. An empty cell shows a dot for each mark that MRK-4 shows, with the current `auto_hide_on`. The tap area of the button is x 1289–1429, y y0+55 to y0+125, with the two ends included. The tap area of the bin is x 1441–1507, the full height of the row. The 12 px between these two areas operate nothing. | R§9.3, owner approval 2026-11-06 |
| CKP-8 | For 0.5 s after a delete of a checkpoint, while `panel = Checkpoints`, a press in the area of the panel at its maximum size changes nothing: the game, `checkpoints` and `panel` do not change. This area has the x range of NAV-18 and y 560–1572 in all layouts. A press out of this area, and a press on the toolbar, operate as usual. Thus the second tap of a fast double tap does not delete the row that moved up and does not close the panel. | R§9.3, owner decision 2026-11-06 |
| CKP-9 | `completed ⇒ panel ≠ Checkpoints`, the bookmark is gray 70, `checkpoints` does not change, and a tap on the bookmark changes nothing. This is also true when a different panel is open: that panel stays open (the exception to NAV-12). The list stays in the game file until a new game (UND-6). `¬completed ⇒` the bookmark is black, or white on black while the panel is open (NAV-8). If a tap on the bookmark makes the app read waiting ink that completes the puzzle, the panel does not open. | R§8.3, R§9.3, owner decision 2026-11-06 |

## 11. Difficulty

| ID | Rule | Source |
|---|---|---|
| DIF-1 | `game.mode`, `settings.last_mode` and `slider` are in `0..6`. There are seven modes: 0 is Easy and 6 is Hard. | R§11 (owner decision 2026-10-25) |
| DIF-2 | At the first start, before the player selects a mode, `last_mode = 2` and the first puzzle has mode 2. | R§11 (confirmed again 2026-10-25) |
| DIF-3 | When the New game panel opens, `slider = last_mode`. | R§11 |
| DIF-4 | When you move the slider, only `slider` changes. The game, `last_mode` and the meter do not change. If the panel closes before Start new game, the app does not keep `slider`. | R§11 |
| DIF-5 | Start new game sets `last_mode = slider`. It makes a game with `mode = slider`, a new seed, `elapsed = 0`, empty stacks, `checkpoints = ∅`, `completed = false`, no entries and no marks. | R§11, R§9.3, owner approval 2026-11-06 |
| DIF-6 | `¬completed ⇒` the meter below the board shows `game.mode`. When the game is complete, "Completed in …" replaces the meter (CMP-4). The player cannot change the meter. | R§3.1, R§8.3, R§11 |
| DIF-7 | Start new game never asks for a confirmation. It never records the old game in `results` (CMP-5). | R§11 |

## 12. Cell colors

| ID | Rule | Source |
|---|---|---|
| COL-1 | `color_starting`, `color_clash`, `color_highlight` and `color_completed` each always have one color from the Palette. The defaults are Gray, Red, Cyan and Green. | R§10, owner decision 2026-11-05 |
| COL-2 | In the Colors panel, each section shows one selected color (black box). That color is equal to the related setting. | R§10 |
| COL-3 | A tap on a color changes only the setting of that section. The board shows the new color immediately on each cell that has that color (BRD-7). | R§10, owner decision 2026-11-05 |
| COL-4 | Color changes do not change the game state or the undo stack (UND-2). | R§7, R§10 |
| COL-5 | `color_starting`, `color_clash`, `color_highlight` and `color_completed` are all different, except White. All four can be White. A color that another of these four sections uses shows as not available, and you cannot select it. White is always available. This rule does not apply to the Micro-cell lines section (COL-7). | R§10, owner decision 2026-11-05 |
| COL-6 | Black is never in the palette. | R§10 |
| COL-7 | `micro_line` always has one of the nine values of the Micro-cell lines section: None, gray90, gray80, gray70, gray60, gray55, gray50, gray45 or gray40. The default is gray90, also for a settings file of an earlier version. The section shows the selected value (black box). A tap on a value changes only `micro_line`, and the board shows the new lines immediately. | R§10 (owner decision 2026-11-02) |
| COL-8 | A group is a completed group exactly when all its nine cells are filled and no cell of the group is clashing. A clash with a cell outside the group also stops the group. An empty cell is never in a completed group. | R§4.2, R§6.3, owner decision 2026-11-05 |
| COL-9 | The set of completed groups is a function of `value` only. The app does not use `solution` for it, does not keep it as a state and does not save it. Each completed group has the digits 1–9 one time. A completed group can have an entry with `value ≠ solution`. A group with only starting numbers that is full at the start of a game is a completed group from the first frame. | R§4.2, R§6.3, owner decision 2026-11-05 |
| COL-10 | After each change (a new number, an erasure, a Restore, undo, redo (also of a Restore) and a new game), a cell has the completed color exactly when `shows_completed` and no earlier item of BRD-7 applies. This is also true for cells that are not peers of the changed cell: an erasure can remove a clash, and then a different group of the other clashing cell becomes a completed group. A group that gets or loses the color does not change the undo and redo stacks, the clock, `results` or `completed`. | R§4.2, R§6.3, owner decision 2026-11-05, R§9.3, owner approval 2026-11-06 |
| COL-11 | `completed_color_on` and the selection of White in the Completed groups section only select whether the completed color shows. With `¬completed_color_on` or `color_completed = White`, no cell has the completed color, and a starting number keeps its starting color (BRD-7). The two conditions give the same board. They do not change the set of completed groups. With `¬completed_color_on`, the Completed groups section stays operative, and `color_completed` stays not available for the other sections (COL-5). | R§9.1, R§10, owner decision 2026-11-05 |
| COL-12 | `completed ⇒` no cell has the completed color. At completion, the board shows the starting color for starting numbers and white for entries. This is also true when a saved completed game opens. | R§8.3, owner decision 2026-11-05 |

## 13. Manual

| ID | Rule | Source |
|---|---|---|
| MAN-1 | `screen = Manual ⇒ manual_page ∈ 1..9`. When the manual opens, `manual_page = 1`. | R§12 |
| MAN-2 | Next (› or swipe left) is available only if `manual_page < 9`. Previous (‹ or swipe right) is available only if `manual_page > 1`. An arrow that is not available is gray and has no effect. | R§12 |
| MAN-3 | "< Back" is on each page and shows Game again. | R§12 |
| MAN-4 | The manual does not change the game. The clock continues to operate (CLK-1). | R§8.4 |

### 13.1 Third-party licenses

| ID | Rule | Source |
|---|---|---|
| LIC-1 | A tap on the row "Third-party licenses" (row 5 of the Settings panel) sets `screen = Licenses`, `licenses.page = 1`, `menu = Closed`. It does not change the game or the settings. | R§9.4 |
| LIC-2 | On Licenses, `licenses.page ∈ 1..pages`, with `pages = ⌈entries / rows⌉` (14 rows in portrait, 9 in landscape). Next and previous (arrows, finger swipe) do not go past the first or the last page. | R§9.4 |
| LIC-3 | A tap on row i of page p sets `screen = LicenseText`, `open` = the entry of that row, `text_page = 1`. On LicenseText, `text_page ∈ 1..text_pages(open)`. "< Back" sets `screen = Licenses` and keeps `licenses.page`. | R§9.4 |
| LIC-4 | Each line of license text has 80 characters or fewer. Each entry has at least one license file with text. | R§9.4 |
| LIC-5 | `assets/licenses.json` agrees with `scripts/licenses.py` (the crates of `cargo metadata` for the tablet target, and their license files). | R§9.4 |

## 14. Statistics and the results database

| ID | Rule | Source |
|---|---|---|
| ANA-1 | Each result has `mode ∈ 0..6`, `seconds ≥ 0`, `givens ∈ 0..81`, a `seed`, a `finished_at`, a `generator` version and a unique `id`. When the app opens, it does not use a row in the results file that breaks this rule. The row stays in the file. | R§14.1 (confirmed 2026-10-25) |
| ANA-2 | Only completion adds results (CMP-5). Only a confirmed Delete (one game) or a confirmed Delete all games removes results. (The device test tool `sudoku --test-results` is not a part of the app; refer to DECISIONS.md.) | R§14.1, R§13.1, R§13.2 |
| ANA-3 | "Puzzles completed" on the summary is `|results|`. "Average time" is the average of `seconds` in `results`, or "–" if there are no results. | R§13.1 |
| ANA-4 | For each mode m, completed is `|{r ∈ results : r.mode = m}|`. Average is the average of their `seconds`, or "–" if there are none. The sum of the quantities for all modes is equal to the total. | R§13.1 |
| ANA-5 | The list shows only the results that agree with `filter` (all results for All). The newest `finished_at` is first. Each page has a maximum of `n` results (`n` in ANA-6). All pages before the last page have `n` results. If no results agree with `filter`, the list has no rows. | R§13.2 |
| ANA-6 | `1 ≤ page ≤ max(1, ⌈matching / n⌉)`, with `n = 12` in portrait and `n = 6` in landscape. When the list opens, `filter = All` and `page = 1`. A change of filter sets `page = 1`. | R§13.2 |
| ANA-7 | `confirm ≠ None ⇒` the dialog shows on the pale list, and the list accepts no other input. Cancel sets `confirm = None` and makes no change. Delete removes that one result and then sets `confirm = None`. | R§13.2 |
| ANA-8 | After a deletion (one game or all games), the summary and the list show the change immediately (ANA-3 to ANA-5 are true). If the current page becomes empty, the list shows the page before it, but not below page 1. Then ANA-6 is true. | R§13.2 |
| ANA-9 | Times always have the format `m:ss`, or `h:mm:ss` after one hour. | R§8.3, R§13.2 |
| ANA-10 | Statistics does not change the game. The clock continues to operate. | R§8.4 |
| ANA-12 | `confirm_all ⇒ screen = Statistics` and the summary accepts no other input. Cancel sets `confirm_all = false` and makes no change. Delete removes all results (`results = ∅`) and sets `confirm_all = false`. If `results = ∅`, "Delete all games" is gray and a tap on it has no effect. | R§13.1 |
| ANA-11 | `finished_at` is Unix time in seconds (UTC). The app shows it in the local time of the device. | R§14.1 (confirmed 2026-10-25) |

## 15. Saved data

| ID | Rule | Source |
|---|---|---|
| PER-1 | At each point where the app can close or be suspended, the saved game is equal to the game in memory: `seed`, `mode`, the `generator` version, starting numbers, `value`, `marks`, the two stacks, `checkpoints`, `elapsed` and `completed`. | R§14.3, R§9.3, owner approval 2026-11-06 |
| PER-2 | When the app opens again, it shows the state of PER-1 exactly. It shows the Game screen with `menu = Open`, `panel = None`, `highlight = None`, `question = None`, `refused = None` and `pending_ink = None`. The app does not keep the menu state. This is true for a new game and for a saved game. `checkpoints` is the same as before the close (CKP-5 gives the exception for a game file of an earlier version). | R§9, R§14.3 (owner decision 2026-11-02), R§9.3, owner approval 2026-11-06 |
| PER-3 | The settings (`color_*`, `micro_line`, `last_mode`, `highlight_on`, `auto_hide_on` and `completed_color_on`) stay the same when the app closes and opens again. | R§14.2, owner decision 2026-11-05 |
| PER-4 | `results` stays the same when the app closes and opens again. Only ANA-2 changes it. | R§14.1 |
| PER-5 | All saved data agrees with all rules in this document. If a save does not agree with a rule, this is a bug. When the app opens, it does not use saved data that breaks a rule (PUZ-5, ANA-1). It uses the default settings in place of a settings file that breaks COL-5 (also PER-8). A saved history that play cannot make, but that agrees with UND-3, is correct: each undo and redo item still changes the board. CKP-5 gives the checks of the checkpoints and of the Restore items. | — (confirmed 2026-10-25; settings: review finding; checkpoints: R§9.3, review finding 2026-11-06) |
| PER-6 | If the app closes during the recognition pause, the app removes the ink. The app never reads it later. | R§14.3 |
| PER-7 | The app saves the game after each change to the board and after each save and each delete of a checkpoint, the settings after each change, and the clock at least each 5 seconds. It also saves when it closes or gets SIGTERM, SIGINT or SIGHUP. A power loss can lose a maximum of 5 seconds of the clock. | R§14.3 (confirmed 2026-10-25), R§9.3, owner approval 2026-11-06 |
| PER-8 | When a settings file has no `color_completed`: (1) If its three colors break COL-5, the app uses the default settings (PER-5). (2) Else, if `color_highlight = Green`, it becomes Cyan, or the first free color if another section uses Cyan. A highlight of another color, also White, does not change. (3) Then `color_completed` becomes Green, or the first free color if another section uses Green. (4) `completed_color_on = true`. The first free color is the first color in the order of the Palette (§0, also the order of the panel) that is not White and that no other section uses. A settings file that has `color_completed` does not get these changes, also with `color_highlight = Green`. A settings file that has `color_completed` and no value for the switch gets `completed_color_on = true`, and its colors do not change. Each start gives the same result from the same file. The app changes the file only at the next change of the settings. | R§14.2; part (1): PER-5 and a review finding, not an owner decision; no value for the switch: default of §0; other parts: owner decision 2026-11-05 |

## 16. Updates

| ID | Rule | Source |
|---|---|---|
| UPD-1 | `state = Off` exactly when `current` is not a stable version (`v<major>.<minor>.<patch>`) and not a preview version (`pre-` and 7 to 40 hexadecimal digits), for example `dev` or `dev/update-icons`. When `state = Off`, the app never sends a request, and the Settings panel has only five rows. In all other states, the Settings panel has six rows. | R§9.1, R§9.2, owner decision 2026-11-05 |
| UPD-2 | `Available(v)`, `Installing(v)` and `Failed(v, at)` only when `v` is a stable version that is an update for `current`: newer than `current` for a stable build, any stable version for a preview build. | R§9.2 (owner decision 2026-11-01) |
| UPD-3 | The gear has the caution icon exactly when `state ∈ {Available, Installing, Failed, Installed}`. | R§9.2 |
| UPD-4 | The app uses a `version.json` only when its ed25519 signature is correct for the public key of the app. It installs an archive only when its SHA-256 checksum is equal to the checksum in that `version.json`. When `version.json` has `patch_sha256`, it changes no file unless the checksum of the downloaded ink patch is equal to `patch_sha256`. | R§9.2 |
| UPD-5 | Only a tap on the button in `state = Available(v)` starts an installation. The tap sets `state = Installing(v)`, and it does not change the game, the settings, the screen or the panel. At most one installation is in progress. | R§9.2 |
| UPD-6 | When step 1, 2, 3 or 4 of an installation fails, the app folder is the same as before the installation. When step 6 fails, the app folder is the new folder. `Failed(v, at)` becomes `Available(v)` 3 seconds after `at`. | R§9.2 |
| UPD-7 | A result of a request changes only `Unknown`, `CheckFailed`, `UpToDate` and `Available`. A failed request changes only `Unknown` (to `CheckFailed`). At most one request is in progress (`checking`). | R§9.2 |
| UPD-8 | The app does not keep `update`. When it opens, `state` is `Off` or `Unknown`, and a stable or a preview build sends a request. `state = Unknown` only while this request is in progress. The app never sends a request by itself after this request. | R§9.2 (owner decision 2026-10-31) |
| UPD-9 | The text "Sudoku is up to date" shows exactly when `up_to_date_shown`. `up_to_date_shown` becomes true only when a request finds no update while the Settings panel is open, and it becomes false when the panel closes (or the screen changes). It is true only when `state = UpToDate`, `checking` is false and the Settings panel is open. The button "Check for updates" shows exactly when `state ∈ {CheckFailed, UpToDate}`, `checking` is false and `up_to_date_shown` is false. A tap on it sets `checking` and sends one request. It does not change the game, the settings, the screen or the panel. `checking` is true only when `state ∈ {Unknown, CheckFailed, UpToDate}`. | R§9.2 (owner decision 2026-10-31) |
| UPD-10 | An installation replaces the ink patch exactly when `version.json` has a patch and the stamp file next to the patch is missing, is not correct, or has a different checksum. Then `state = Installed { patch: true }`, else `Installed { patch: false }`. `dialog` ("Restart the interface?") becomes true only when an installation replaced the patch, and it is true only in `Installed { patch: true }`. While `dialog` is true, no ink waits, no stroke is live, and only "Later" and "Restart" have an effect: "Later" sets `dialog` to false, and "Restart" asks the platform to run xovi/start after the game is saved. The button "Restart interface" shows exactly when `state = Installed { patch: true }`. A tap on it asks the platform to run xovi/start after the game is saved, and it does not change the game, the settings, the screen or the panel. The app asks for xovi/start only in `Installed { patch: true }`. | R§9.2 (owner decision 2026-11-01) |

## 17. Rules to confirm

There are no rules to confirm now. If a rule in this document comes from a conclusion and not from a
statement in `REQUIREMENTS.md`, put it in this section until the rule is confirmed.
