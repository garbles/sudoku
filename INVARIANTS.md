# Sudoku for reMarkable Paper Pro — State invariants

This document is written in ASD-STE100 Simplified Technical English (STE).

This document gives the rules that must be true **in all states that the app can get to**. The rules
come from [`REQUIREMENTS.md`](REQUIREMENTS.md). You can examine each rule with an assertion in the
code, a property-based test, or a check of the saved data.

If a rule in this document and the requirements do not agree, the requirements are correct. Then
correct this document.

Each rule has an ID (for example `BRD-3`). Each rule also gives the section of the requirements that it
comes from (for example *R§4.1*). Rules that are not confirmed yet are in
[§16](#16-rules-to-confirm).

## Contents

0. [State model](#0-state-model)
1. [Puzzle and solution](#1-puzzle-and-solution)
2. [Cells](#2-cells)
3. [Pencil marks](#3-pencil-marks)
4. [Clashes](#4-clashes)
5. [Overlays: highlight and "?"](#5-overlays-highlight-and-)
6. [Input](#6-input)
7. [Undo and redo](#7-undo-and-redo)
8. [Completion and the lock](#8-completion-and-the-lock)
9. [Clock](#9-clock)
10. [Screens, menu and panels](#10-screens-menu-and-panels)
11. [Difficulty](#11-difficulty)
12. [Cell colors](#12-cell-colors)
13. [Manual](#13-manual)
14. [Statistics and the results database](#14-statistics-and-the-results-database)
15. [Saved data](#15-saved-data)
16. [Rules to confirm](#16-rules-to-confirm)

---

## 0. State model

The rules use this model of the state of the app. The names are examples. The code can use different
names if it has an equivalent state.

```
App
  screen          ∈ {Game, Manual, Statistics, GamesList}
  game            : Game
  ui              : GameUI
  manual_page     ∈ 1..9                       (used when screen = Manual)
  list            : ListUI                     (used when screen = GamesList)
  confirm_all     ∈ {false, true}              (used when screen = Statistics: the "Delete all games" dialog)
  settings        : Settings
  results         : set of Result               (the SQLite table)

Game
  seed            : integer                    (from the generator; no fixed range)
  mode            ∈ 0..6
  generator       ∈ {1, 2}                     (the version of the generator; 1 only for mode ∈ 0..4)
  solution[r][c]  ∈ 1..9                       for r, c ∈ 0..8
  given[r][c]     ∈ {true, false}
  value[r][c]     ∈ 0..9                       (0 = empty)
  marks[r][c]     ⊆ {1..9}                     (pencil marks; kept also when hidden)
  undo, redo      : stacks of Change
  elapsed         : seconds ≥ 0
  completed       : bool

GameUI
  menu            ∈ {Closed, Open}
  panel           ∈ {None, Colors, Settings, NewGame}
  highlight       ∈ {None} ∪ 1..9
  question        ∈ {None} ∪ (cell, shown_at)   (the temporary "?")
  pending_ink     ∈ {None} ∪ (cell, strokes, last_lift_at)
  slider          ∈ 0..6                       (the selection in the New game panel)

ListUI
  filter          ∈ {All} ∪ 0..6
  page            ≥ 1
  confirm         ∈ {None} ∪ Result             (the delete dialog)

Settings
  color_starting, color_clash, color_highlight ∈ Palette
  last_mode       ∈ 0..6
  highlight_on    : bool                       (Settings panel; default true)
  auto_remove_on  : bool                       (Settings panel; default true)

Palette = {Gray, White, Yellow, Blue, Red, Green, Cyan, Magenta, Orange}

Result
  id, finished_at, mode ∈ 0..6, givens ∈ 0..81, seconds ≥ 0, seed, generator
```

Terms:

- **peers(r, c)**: the 20 other cells in row r, in column c and in the 3×3 box of (r, c).
- **filled(r, c)**: `value[r][c] ≠ 0`.
- **entry(r, c)**: the cell is filled and is not a starting number ("your number").
- **clashing(r, c)**: the cell is filled and a peer has the same value.
- **can_undo / can_redo**: the stack is not empty and the game is not complete.

## 1. Puzzle and solution

| ID | Rule | Source |
|---|---|---|
| PUZ-1 | `solution` is a complete, correct sudoku. Each row, column and box has the digits 1–9 one time. | R§14.5 |
| PUZ-2 | The starting numbers have **only one** solution. That solution is `solution`. | R§14.5 |
| PUZ-3 | For each starting number, `value = solution`. Thus two starting numbers never clash. | R§14.5 |
| PUZ-4 | `given` does not change after the generator makes the puzzle. It changes only when a new puzzle replaces the game. | R§4.1 |
| PUZ-5 | `(seed, mode)` and the version of the generator set the full game. The same values always give the same starting numbers and the same solution. A saved game whose starting numbers or solution are not `generate_version(generator, seed, mode)` is not correct: the app does not use it (PER-5). A new game has the current version (2). Version 1 has only the modes 0..4. A game file with no version is version 1. | R§14.1, R§14.5 (confirmed 2026-10-25; versions: owner decision 2026-10-25) |
| PUZ-6 | The generator sets the level (the most difficult group of techniques) of each mode. The human-style solver, with all techniques, completes each puzzle, and the most difficult technique that it needs is of the level of the mode. Easy has 44 starting numbers. No other part of the app uses a fixed level or a fixed quantity of starting numbers. | R§11, R§14.5 (owner decision 2026-10-25) |

## 2. Cells

| ID | Rule | Source |
|---|---|---|
| BRD-1 | Each cell is always one of these types: starting number (`given`), your number (`entry`), empty with no marks, empty with marks. | R§4.1 |
| BRD-2 | `given ⇒ filled`. The value of a starting number does not change. | R§4.1 |
| BRD-3 | The value of an entry is 1–9. Only the eraser or undo can remove it. Writing does not replace it. | R§4.1, R§5.1 |
| BRD-4 | A value changes from 0 to a digit only by a number from 1 to 9 that the app reads, by redo, by undo of an erasure, or by the generator. | R§5.1, R§7 |
| BRD-5 | An entry can be incorrect. `value ≠ solution` is a correct state and has no color, if there is no clash. | R§6.3 |
| BRD-6 | Filled cells do not show the micro-cell grid or pencil marks. Empty cells show the micro-cell grid. The only exception is an empty cell with a "?" (QST-1): it does not show the grid while the "?" shows. | R§4.1, R§5.1 |
| BRD-7 | The background of a filled cell is the **first** of these: the clash color if clashing; the highlight color if `value = highlight`; the starting color if given; white. Empty cells are white. | R§4.2 |
| BRD-8 | Starting digits are black. Entry digits are dark gray (`gray35`). | R§4.1, R§15 |

## 3. Pencil marks

| ID | Rule | Source |
|---|---|---|
| MRK-1 | `marks[r][c] ⊆ {1..9}`. Each mark is at its keypad position (1 at the top left, 9 at the bottom right). | R§3.1 |
| MRK-2 | Marks **show only when the cell is empty**. A filled cell can have marks, but they are hidden. | R§4.1 |
| MRK-3 | Starting numbers never have marks. You can add marks only to empty cells, and starting numbers are never empty. | R§4.1 |
| MRK-4 | **Automatic removal:** if `auto_remove_on`, after you add a number d at (r, c), no peer of (r, c) has d in its marks. This applies to visible marks (empty peers) and to hidden marks (filled peers). | R§6.1 |
| MRK-5 | Marks change only in these ways: a pen tap adds or removes one mark in an empty cell; a new number removes marks (MRK-4); undo and redo of these changes. The eraser never changes marks. | R§5.1, R§6.1, R§7 |
| MRK-6 | When you erase or undo a number, the hidden marks of that cell show again with no change. | R§4.1 |
| MRK-7 | A mark does not have to be a possible digit. You can add a mark for a digit that is already in a peer. The app removes marks only when you add a number. | R§6.1 |
| MRK-8 | When you erase a number, the app does not put back the marks that the number removed from other cells. Only undo puts them back (UND-4). | R§6.1 |
| MRK-9 | If `¬auto_remove_on`, a new number does not change the marks of any other cell. The change has no removed marks, and its undo and redo change only that cell. | R§6.1, R§9.1 |

## 4. Clashes

| ID | Rule | Source |
|---|---|---|
| CLS-1 | The set of clashing cells is always `{(r, c) : clashing(r, c)}`. The app calculates it from `value` after each change: a new number, an erasure, undo, redo and a new game. The app does not keep it separately from the board. | R§6.3 |
| CLS-2 | Clashes go in two directions. If (r, c) clashes with a peer, that peer also clashes. Both cells get the clash color. | R§6.3 |
| CLS-3 | Two starting numbers never clash (PUZ-3). Thus each clash includes one or more entries. | R§6.3, R§14.5 |
| CLS-4 | The app never compares the board with `solution` for the player. An incorrect entry with no clash looks the same as a correct entry. | R§6.3 |

## 5. Overlays: highlight and "?"

| ID | Rule | Source |
|---|---|---|
| HLT-1 | `highlight ≠ None ⇒` the game is not complete. | R§6.2, R§8.3 |
| HLT-2 | `highlight ≠ None ⇒ highlight` is the value of the last number that you tapped, and that tap was the last action. Each other action sets `highlight = None` before the app does the action. Other actions are a pen tap or stroke, a finger touch, a menu or panel action, and a change of screen. | R§3.2, R§6.2 |
| HLT-3 | A tap on a number with a value equal to `highlight` sets `highlight = None`. If `highlight_on`, a tap on a number with a different value sets `highlight` to that value (HLT-6). | R§6.2 |
| HLT-4 | `highlight ≠ None ⇒ screen = Game and panel = None`. When the menu opens or the screen changes, the app removes the highlight. While the menu is open and no panel is open, a pen tap on a number can make a new highlight. | R§9, R§6.2 |
| HLT-5 | A highlight does not change `value`, `marks`, the undo and redo stacks, or `completed`. | R§7 |
| HLT-6 | `¬highlight_on ⇒ highlight = None`. A tap on a number then changes nothing (it still removes a highlight and a "?" as each action does). | R§6.2, R§9.1 |
| QST-1 | `question ≠ None ⇒` the cell of the "?" is empty. The "?" shows for approximately one second. Then `question = None`. While the "?" shows, the marks of the cell are hidden. After the "?", the cell is the same as before. | R§5.1 |
| QST-2 | The "?" does not change `value`, `marks` or the undo and redo stacks. | R§5.1, R§7 |
| QST-3 | `question ≠ None ⇒` the game is not complete. | R§3.2 |
| QST-4 | Each press of the pen or a finger removes the "?" immediately. Thus a new number in that cell never shows under the "?" (QST-1). | R§5.1 (confirmed 2026-10-25) |

## 6. Input

| ID | Rule | Source |
|---|---|---|
| INP-1 | Finger touches on the board do not change the game state. The only exception is the removal of a highlight (HLT-2). A finger can operate all items that are not on the board: the menu buttons, the panels, the dialogs and the screen buttons. In the manual and the games list, a horizontal finger swipe can change the page. | R§5.1, R§12, R§13.2 |
| INP-2 | A pen touch is a **tap** only if all of its movement stays in one micro-cell. All other pen touches are writing. Each pen touch is one of these: a tap, writing, or the eraser. | R§5.1 |
| INP-3 | A tap on an empty cell adds or removes one mark: the digit of the micro-cell that you tapped. | R§5.1 |
| INP-4 | A tap on a filled cell does not change the board. It changes only `highlight` (HLT-3, HLT-6). | R§5.1, R§6.2 |
| INP-5 | `pending_ink ≠ None ⇒` its cell is empty and the game is not complete. The app collects the ink until the pen is lifted for the recognition pause (approximately 0.5 s). Then the app reads all the ink as one item and sets `pending_ink = None`. | R§5.1 |
| INP-6 | The app reads the ink as one of these two results: a number from 1 to 9 (the app adds it), or "not 1–9" (scribble, letter, 0, 10 or more). For "not 1–9", the app shows the "?". There is no other result. | R§5.1 |
| INP-7 | The app removes writing on a filled cell (starting number or entry). The writing makes no change. | R§5.1 |
| INP-8 | The eraser changes the board only on an entry. It makes the cell empty and shows its hidden marks again. On starting numbers and empty cells, it makes no change. The eraser on the cell of the waiting ink removes the ink. The app does not read that ink, thus the board and the undo history do not change. | R§5.1 (confirmed 2026-10-25) |
| INP-9 | When `completed` is true, pen input does not change `value`, `marks`, `highlight` or the stacks. | R§8.3 |
| INP-10 | Ink never stays on the board. After the app reads it, removes it or shows the "?", the board shows only typed numbers, marks and the grid. | R§5.1 |
| INP-11 | While ink waits in a cell, each pen touch that starts in the same cell is writing. This is also true if the touch stays in one micro-cell. Thus a short stroke of a number (for example the bar of a 7) does not become a pencil mark. | R§5.1 (confirmed 2026-10-25) |
| INP-12 | Writing belongs to the cell where the pen touch started. A pen touch that starts in a different cell, or a press on a different item, makes the app read the waiting ink immediately. | R§5.1 (confirmed 2026-10-25) |
| INP-13 | The eraser removes all of your numbers that it touches in one stroke. Each removal is one change in the undo history. | R§5.1, UND-1 (confirmed 2026-10-25) |
| INP-14 | A finger swipe is a horizontal movement of 120 px or more, with more horizontal than vertical movement. | R§12, R§13.2 (confirmed 2026-10-25) |
| INP-15 | A finger press on an item that is not on the board (INP-1: menu buttons, panels, dialogs, screen buttons) operates it at all times while the app is open. If the app does not get the release of a touch, that touch does not stop other presses: the app forgets it when a new press with the same touch id starts, or when a new touch starts and that touch has had no event for 0.15 s. A second finger while the first finger moves or presses (an event less than 0.15 s before) has no effect. | R§9, R§5.1 |

## 7. Undo and redo

| ID | Rule | Source |
|---|---|---|
| UND-1 | Each change to the board is one of these: add or remove a mark; add a number (with the marks that it removes, as one change); erase a number. Each change adds one item to `undo` and empties `redo`. | R§7 |
| UND-2 | No other action adds to `undo`. Highlights, the "?", color changes, switch changes, panels and screens never add to it. | R§7 |
| UND-3 | Undo does the exact opposite of the top change and moves the change to `redo`. Redo does the change again and moves it back to `undo`. An undo and then a redo give the same board as before. A redo and then an undo also give the same board. | R§7 |
| UND-4 | Undo of a number puts back all marks that the number removed (MRK-4). It also shows the hidden marks of that cell again. | R§6.1 |
| UND-5 | After each undo or redo, CLS-1 is true. The app calculates the clashes again. | R§6.3 |
| UND-6 | A new game sets `undo = redo = ∅`. | R§7 |
| UND-7 | The undo button is black only if `can_undo`. The redo button is black only if `can_redo`. In all other conditions, the button is gray and has no effect. | R§7 |
| UND-8 | `completed ⇒` the two buttons are gray and have no effect. This is also true when the stacks are not empty. | R§7, R§8.3 |
| UND-9 | Undo and redo do not change `given`, `solution`, `seed`, `mode` or `elapsed`. | R§7 |

## 8. Completion and the lock

| ID | Rule | Source |
|---|---|---|
| CMP-1 | `completed ⇔` all cells are filled and no cell clashes. The app sets it to true immediately when this becomes true. Only a new game sets it to false. | R§8.1, R§8.3 |
| CMP-2 | Each row, column and box of a complete board has the digits 1–9 one time. Thus a complete board is the same as `solution` (PUZ-2). | R§8.3, R§14.5 |
| CMP-3 | `completed ⇒` the board, the marks and the stacks do not change (INP-9, UND-8), and `highlight = None`, `question = None`, `pending_ink = None`. | R§8.3 |
| CMP-4 | `completed ⇒` the text "Completed in" and `elapsed` replace the meter below the board. The format is `m:ss`, and `h:mm:ss` after one hour. `¬completed ⇒` the meter shows `mode`. The player cannot change the meter. | R§3.1, R§8.3 |
| CMP-5 | Completion adds **one** row to `results`. The row has the `mode`, the quantity of starting numbers, the final `elapsed`, the `seed` and the `generator` version of this game. The app never adds a row for a game that is not complete. | R§8.3, R§14.1 |
| CMP-6 | Only Start new game ends the `completed` state. Help, Statistics, colors, settings and the menu continue to operate. | R§8.3 |

## 9. Clock

| ID | Rule | Source |
|---|---|---|
| CLK-1 | `elapsed` increases only when the app is open (on all screens) and the game is not complete. | R§8.4 |
| CLK-2 | `elapsed` never decreases. The only exception is a new game, which sets it to 0. | R§8.4 |
| CLK-3 | `completed ⇒ elapsed` does not change. It is the final time and is equal to `seconds` in the saved result. | R§8.4, R§14.1 |
| CLK-4 | The app does not show the clock during the game. It shows the time only in "Completed in …". | R§8.4 |
| CLK-5 | The app does not count the time when the app is closed or suspended. When the app opens again, the clock continues from the saved `elapsed`. | R§8.4, R§14.3 |
| CLK-6 | If the app gets no events and no clock ticks for more than 5 seconds, the clock does not count that time (the app was suspended). The app ticks at least one time each second while it operates. | R§8.4 (confirmed 2026-10-25) |

## 10. Screens, menu and panels

| ID | Rule | Source |
|---|---|---|
| NAV-1 | The app shows one screen at a time: Game, Manual, Statistics or GamesList. The app starts on Game. | R§2 |
| NAV-2 | Only the open menu opens Manual and Statistics. Only Statistics opens GamesList. "< Back" goes from Manual to Game, from Statistics to Game, and from GamesList to Statistics. | R§2 |
| NAV-3 | When Manual or Statistics shows Game again, `menu = Closed` and `panel = None`. | R§12, R§13.1 |
| NAV-4 | `panel ≠ None ⇒ menu = Open`. Only one panel can be open at a time. | R§9 |
| NAV-5 | `screen ≠ Game ⇒ menu = Closed, panel = None`. The menu is a part of the game screen. | R§9 |
| NAV-6 | The close button shows only when `screen = Game` and `menu = Open`. A tap on it closes the app after the app saves the game (PER-1). | R§3.1, R§9 |
| NAV-7 | Icon of the menu button: the dot is at the bottom left only when `menu = Closed`. The dot is at the top left only when `menu = Open`. | R§3.1 |
| NAV-8 | The button of a panel is white on black only when that panel is open. The other buttons have their usual appearance. | R§9 |
| NAV-9 | After Start new game, `menu = Closed` and `panel = None`. | R§11 |
| NAV-10 | The closed menu never covers the board. The open toolbar (portrait x ≥ 1508, landscape x ≥ 2048) never covers the board (portrait x ≤ 1471, landscape x ≤ 1741). An open panel can cover a part of the board. | R§3.1, R§3.3 |
| NAV-15 | When the device is right-handed, the game screen shows the toolbar, the menu button, the close button and the panels as a mirror image of the left-handed screen: the toolbar is at the left edge (x 0–112), the close button is at the top right, and the panels touch the toolbar at x 112. The icons other than the menu button and the contents of the panels are the same. The hit areas move with the buttons. | R§3.1 (owner request 2026-10-26) |
| NAV-13 | A turn of the tablet does not change the game, the settings or the screen. It removes only the pending ink and the touches in progress. | R§3.3 (confirmed 2026-10-25) |
| NAV-14 | The Settings panel opens from the gear (top slot 2). A tap on a row changes only its switch (`highlight_on` or `auto_remove_on`), immediately; the panel stays open. A tap on the version row (row 3) changes nothing, and the panel stays open. A change of a switch does not change the game or the undo and redo stacks, and it does not change the changes that are already in the stacks. | R§9.1 (version row: owner decision 2026-10-26) |
| NAV-11 | When `panel ≠ None`, a tap outside the panel only sets `panel = None`. It does not change the game, also when the tap is on the board. When `menu = Open` and `panel = None`, the board operates as usual. | R§9 |
| NAV-12 | When `panel ≠ None`, a tap on a toolbar button that is not the paint bucket, the gear or the + only closes the panel. A tap on the button of a different panel opens that panel. This includes the menu button and the close button. | R§9 (confirmed 2026-10-25) |

## 11. Difficulty

| ID | Rule | Source |
|---|---|---|
| DIF-1 | `game.mode`, `settings.last_mode` and `slider` are in `0..6`. There are seven modes: 0 is Easy and 6 is Hard. | R§11 (owner decision 2026-10-25) |
| DIF-2 | At the first start, before the player selects a mode, `last_mode = 2` and the first puzzle has mode 2. | R§11 (confirmed again 2026-10-25) |
| DIF-3 | When the New game panel opens, `slider = last_mode`. | R§11 |
| DIF-4 | When you move the slider, only `slider` changes. The game, `last_mode` and the meter do not change. If the panel closes before Start new game, the app does not keep `slider`. | R§11 |
| DIF-5 | Start new game sets `last_mode = slider`. It makes a game with `mode = slider`, a new seed, `elapsed = 0`, empty stacks, `completed = false`, no entries and no marks. | R§11 |
| DIF-6 | `¬completed ⇒` the meter below the board shows `game.mode`. When the game is complete, "Completed in …" replaces the meter (CMP-4). The player cannot change the meter. | R§3.1, R§8.3, R§11 |
| DIF-7 | Start new game never asks for a confirmation. It never records the old game in `results` (CMP-5). | R§11 |

## 12. Cell colors

| ID | Rule | Source |
|---|---|---|
| COL-1 | `color_starting`, `color_clash` and `color_highlight` each always have one color from the Palette. The defaults are Gray, Red and Green. | R§10 |
| COL-2 | In the Colors panel, each section shows one selected color (black box). That color is equal to the related setting. | R§10 |
| COL-3 | A tap on a color changes only the setting of that section. The board shows the new color immediately (BRD-7). | R§10 |
| COL-4 | Color changes do not change the game state or the undo stack (UND-2). | R§7, R§10 |
| COL-5 | `color_starting`, `color_clash` and `color_highlight` are all different, except White. All three can be White. A color that another section uses shows as not available, and you cannot select it. White is always available. | R§10 |
| COL-6 | Black is never in the palette. | R§10 |

## 13. Manual

| ID | Rule | Source |
|---|---|---|
| MAN-1 | `screen = Manual ⇒ manual_page ∈ 1..9`. When the manual opens, `manual_page = 1`. | R§12 |
| MAN-2 | Next (› or swipe left) is available only if `manual_page < 9`. Previous (‹ or swipe right) is available only if `manual_page > 1`. An arrow that is not available is gray and has no effect. | R§12 |
| MAN-3 | "< Back" is on each page and shows Game again. | R§12 |
| MAN-4 | The manual does not change the game. The clock continues to operate (CLK-1). | R§8.4 |

## 14. Statistics and the results database

| ID | Rule | Source |
|---|---|---|
| ANA-1 | Each result has `mode ∈ 0..6`, `seconds ≥ 0`, `givens ∈ 0..81`, a `seed`, a `finished_at`, a `generator` version and a unique `id`. When the app opens, it does not use a row in the results file that breaks this rule. The row stays in the file. | R§14.1 (confirmed 2026-10-25) |
| ANA-2 | Only completion adds results (CMP-5). Only a confirmed Delete (one game) or a confirmed Delete all games removes results. (The device test tool `sudoku --test-results` is not a part of the app; refer to DECISIONS.md.) | R§14.1, R§13.1, R§13.2 |
| ANA-3 | "Puzzles completed" on the summary is `|results|`. "Average time" is the average of `seconds` in `results`, or "–" if there are no results. | R§13.1 |
| ANA-4 | For each mode m, completed is `|{r ∈ results : r.mode = m}|`. Average is the average of their `seconds`, or "–" if there are none. The sum of the quantities for all modes is equal to the total. | R§13.1 |
| ANA-5 | The list shows only the results that agree with `filter` (all results for All). The newest `finished_at` is first. Each page has a maximum of 12 results. All pages before the last page have 12 results. If no results agree with `filter`, the list has no rows. | R§13.2 |
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
| PER-1 | At each point where the app can close or be suspended, the saved game is equal to the game in memory: `seed`, `mode`, starting numbers, `value`, `marks`, the two stacks, `elapsed` and `completed`. | R§14.3 |
| PER-2 | When the app opens again, it shows the state of PER-1 exactly. It shows the Game screen with `menu = Closed`, `panel = None`, `highlight = None`, `question = None` and `pending_ink = None`. | R§14.3 |
| PER-3 | The settings (`color_*`, `last_mode`, `highlight_on` and `auto_remove_on`) stay the same when the app closes and opens again. | R§14.2 |
| PER-4 | `results` stays the same when the app closes and opens again. Only ANA-2 changes it. | R§14.1 |
| PER-5 | All saved data agrees with all rules in this document. If a save does not agree with a rule, this is a bug. When the app opens, it does not use saved data that breaks a rule (PUZ-5, ANA-1). A saved history that play cannot make, but that agrees with UND-3, is correct: each undo and redo item still changes the board. | — (confirmed 2026-10-25) |
| PER-6 | If the app closes during the recognition pause, the app removes the ink. The app never reads it later. | R§14.3 |
| PER-7 | The app saves the game after each change to the board, the settings after each change, and the clock at least each 5 seconds. It also saves when it closes or gets SIGTERM, SIGINT or SIGHUP. A power loss can lose a maximum of 5 seconds of the clock. | R§14.3 (confirmed 2026-10-25) |

## 16. Rules to confirm

There are no rules to confirm now. If a rule in this document comes from a conclusion and not from a
statement in `REQUIREMENTS.md`, put it in this section until the rule is confirmed.
