# Image index

This document is written in ASD-STE100 Simplified Technical English (STE).

These images are the design reference for the sudoku app on the reMarkable Paper Pro. Each image is
1620×2160 (the portrait screen of the Paper Pro, 229 DPI), if the table does not give a different
size. The images are exact. The app must show the same pixels.

Scripts make all of the images. Do not edit the images. To change an image, change the scripts and
run:

```sh
sh scripts/build.sh
```

The file `scripts/ui-lib.sh` has the layout values (board position, toolbar slots, fonts). The
dimensions of the toolbar, the panels and the fonts come from screenshots of a real Paper Pro
(`scripts/rmpp-screenshot.sh`).

[`REQUIREMENTS.md`](../REQUIREMENTS.md) gives the full behavior of the app.

## Game screens

| Image | Content | Purpose |
|---|---|---|
| `screen.png` | Empty board, menu closed. It shows only the menu button (top right, closed icon: dot at the bottom left) and the difficulty meter below the board. | The base layout. The board has 9×9 cells of 147 px, at the center, from (148, 418) to (1471, 1741). Each cell has a faint 3×3 grid of 49 px micro-cells. The close button does not show when the menu is closed. |
| `screen-in-progress.png` | A game in progress: starting numbers (gray background, black digits), your numbers (white background, gray digits), pencil marks and one clash (two red 9s). | The main state of the game. The pencil marks are the possible digits only, because the app removes the other marks automatically. |
| `screen-highlight.png` | The same game after a tap on a 5. All 5s have the highlight color (green). | Highlights. A tap on the same number again, or a different action, removes the highlight. |
| `screen-unrecognized.png` | The same game immediately after a scribble that the app cannot read. A gray **?** shows in the cell for approximately one second. | The response to writing that is not a number from 1 to 9 (for example a scribble, 0 or 12). After the "?", the cell is the same as before. |
| `screen-completed.png` | The complete board. The text "Completed in 23:41" replaces the difficulty meter. | The complete, locked state. |

## Menu (open)

The menu is a column of 112 px on the right. It has the same design as the toolbar of the device.
From the top to the bottom, it has these buttons: the menu button (open icon: dot at the top left),
cell colors, settings (gear), undo and redo. At the bottom, it has Help, Statistics and + (new game). When the menu is
open, the close button shows at the top left.

| Image | Content | Purpose |
|---|---|---|
| `screen-toolbar.png` | The menu is open on a new game. Undo and redo are gray, because there is no change to undo. | Shows all menu buttons and their icons. |
| `new-game-menu.png` | The + button is black, and its panel is open. The panel has the difficulty slider (seven modes) and "Start new game". | How to start a new game. The panel is wider than the menus of the device, so that the slider is easy to use. The slider has the same size as the meter below the board. |
| `color-menu.png` | The Cell colors panel is open on the game in progress. It has three sections: starting numbers (Gray), clashing numbers (Red) and highlighted numbers (Green). | The only place to change the cell colors. Each section has the same design as the pen-color menu of the device: a title, a 3×3 grid of 112 px cells and a black box around the selected color. |
| `color-menu-right-handed.png` | The same state as `color-menu.png`, for a right-handed player. The toolbar is at the left edge (x 0–112), the menu icon is a mirror image (dot at the right), the close button is at the top right and the panel is at x 112–511. | The menu at the other side (REQUIREMENTS.md §3.1). The other icons and the contents of the panel do not change. `scripts/toolbar.sh` makes it from `color-menu.png`. |
| `settings-menu.png` | The Settings panel is open on the game in progress (the gear is white on black). Two rows: "Highlight numbers" and "Remove pencil marks automatically", each with a switch (both on). The third row is "Version" with the version of a development build ("dev", Regular, gray40) at the right. | The only place to turn the two helpers off and on (REQUIREMENTS.md §9.1). |
| `screen-toolbar-update.png` | The menu is open on the game in progress of a stable build (v0.1.0). An update (v0.2.0) is available: the gear has the red caution icon at its top right. | The caution icon (REQUIREMENTS.md §9.2). |
| `settings-update.png` | The Settings panel of the stable build v0.1.0 when v0.2.0 is available. The Version row shows "v0.1.0". The fourth row has the button "Update to v0.2.0" with the update icon. The gear has the caution icon. | The update button (REQUIREMENTS.md §9.2). |
| `settings-up-to-date.png` | The Settings panel of the stable build v0.1.0 when no update is available. The fourth row shows "Sudoku is up to date" (Regular, gray40). No caution icon. | The text states of the update row (REQUIREMENTS.md §9.2). The other texts have the same position and font. |
| `screen-completed-menu.png` | The menu is open on a complete puzzle. Undo and redo are gray. | Shows that a complete puzzle is locked. |

## Statistics

`scripts/analytics.py` also makes the three screens for landscape (`analytics-landscape.png`,
`analytics-games-landscape.png`, `analytics-delete-landscape.png`, `analytics-delete-all-landscape.png`, 2160×1620). The games list then is
one table across the full width, with 6 games on each page.

Statistics in the menu opens these screens (the image files keep the name "analytics"). They have the same top and bottom items as the manual:
"< Back" shows the game again. The two screens use one list of sample results. Thus their numbers
agree.

| Image | Content | Purpose |
|---|---|---|
| `analytics.png` | The quantity of completed puzzles and the average time for all puzzles. Below these is one row for each difficulty (meter, quantity, average time) and the "All completed games" button. | The summary of the results in the SQLite database, with "All completed games" and "Delete all games". If a difficulty has no games, its average is "–". |
| `analytics-games.png` | All completed games, with the newest game first. Each row has the date and time of completion, the difficulty, the time used and a bin to delete the game. A filter row (All, or one difficulty) is above the list. Each page has 12 games, with page arrows. A swipe also turns the page. | How to see and delete results. |
| `analytics-delete.png` | The list after a tap on a bin. A dialog shows "Delete this game?", the date and time of the game, and Cancel and Delete. | A deletion is permanent. Thus the app asks for a confirmation first. Delete removes the result, and the summary shows the change. |
| `analytics-delete-all.png` | The summary after a tap on "Delete all games". A dialog shows "Delete all games?", the quantity of games, and Cancel and Delete. | Delete all removes all results. The app asks for a confirmation first. |

## Parts

| Image | Content |
|---|---|
| `template.png` | A blank canvas of 1620×2160. |
| `box.png` | One 3×3 box with 20 px of space around it, at the pixel scale of the board. |
| `cell.png` | One cell with its micro-cells, with 20 px of space around it, at the same scale. |

## Manual

Each page also has a landscape version (`manual-<page>-landscape.png`, 2160×1620) with the same text
in two columns. `scripts/manual-<page>-landscape.sh` makes it.

Help in the menu opens the manual. Each page has "< Back" at the top left. It shows the game again.
The settings pages of the device have the same item. Each page also has arrows for the previous and
next page, next to the page number.

| Page | Image | Content |
|---|---|---|
| 1 | `manual-how-to-play.png` | The rules, starting numbers and your numbers, the close button and the menu button. |
| 2 | `manual-candidates.png` | Pencil marks: how to tap micro-cells to add and remove them. |
| 3 | `manual-pen.png` | How to write a number (the app reads it and shows a typed number) and how to erase it (the pencil marks show again). |
| 4 | `manual-pen-rules.png` | What a tap is. Finger touches have no effect. One number for each cell. Writing on a number has no effect. Writing that the app cannot read shows **?**. |
| 5 | `manual-helpers.png` | Automatic pencil mark removal and highlights. |
| 6 | `manual-mistakes.png` | Numbers in a clash become red. How to correct them. The app shows only clashes. |
| 7 | `manual-menu.png` | All menu buttons, and how to select a color. |
| 8 | `manual-new-game.png` | How to start a new game, step by step. The difficulty meter, which you cannot change. |
| 9 | `manual-finished.png` | How a puzzle becomes complete, the lock, the clock and the saved results. |

## Installer

The terminal output of `install.sh` (REQUIREMENTS.md §14.6), in the simulation of
`tests/install/run.sh`. `scripts/terminal.py` draws each image (width 1549 px, the height changes
with the output). The font is DejaVu Sans Mono (`scripts/fonts/`).

| Image | Scenario | Content |
|---|---|---|
| `install-first.png` | `first` | A new tablet in developer mode. All 9 steps operate: the SSH key and the password, Vellum, AppLoad, the hashtable, xovi and Sudoku. |
| `install-connect.png` | `connect` | The tablet does not answer (instructions for the USB cable), then SSH is off (instructions for developer mode). All other steps are done already. |
| `install-restart.png` | `restart` | After a restart of the tablet with an earlier version of Sudoku: the installer starts xovi and replaces the app. |
| `install-again.png` | `again` | All steps are done already. The installer changes nothing. |

## Behavior summary

This section is a short summary. `REQUIREMENTS.md` gives the full behavior.

**Input**
- On the board, only the pen has an effect. Finger touches have no effect.
- In the manual and the games list, a finger swipe to the left or the right turns the page. The page
  arrows also turn the page. reMarkable notebooks operate in the same way.
- A tap is a pen touch that stays in one micro-cell. On an empty cell, a tap adds or removes the pencil
  mark of that micro-cell. On a number, a tap highlights all cells with that number.
- The app reads writing in an empty cell after a short pause (approximately 0.5 s) after the pen
  lifts. Thus a number with more than one stroke is correct.
- If the writing is a number from 1 to 9, the app removes the ink and shows a typed number. The pencil
  marks of that cell are hidden. They show again if you erase the number.
- If the writing is not a number from 1 to 9 (for example a scribble, 0 or 12), the app shows **?** for
  approximately one second. Then the cell is the same as before.
- Writing on a starting number or on your number has no effect. To change a number, erase it first.
- The eraser removes your number and shows the pencil marks of the cell again. It has no effect on
  starting numbers or empty cells.

**Helpers**
- When you write a number, the app removes that digit from the pencil marks in the same row, column
  and box. Undo puts the marks back.
- A highlight gives the highlight color (default: green) to all cells with the same number. A tap on
  the same number again removes the highlight. All other actions also remove it, for example a tap
  on the menu button.
- There are three cell colors: starting numbers (default: gray), clashing numbers (default: red) and
  highlighted numbers (default: green). You select each color from nine medium-light colors in the Cell colors
  panel. Black is not available.
- The three colors must be different. White is the only exception: all three can be White. If a
  section uses a color, the other sections show a line through it.
- If a cell has two colors, the clash color is first, then the highlight color, then the
  starting-number color.
- Numbers in a clash (the same digit in a row, column or box) get the clash color. The app shows only
  clashes. It does not compare the numbers with the solution.

**Puzzle completion**
- A puzzle is complete when all cells have a number and there is no clash.
- The difficulty meter changes to "Completed in m:ss" (h:mm:ss after one hour).
- The board locks. Writing, the eraser, pencil marks, highlights, undo and redo have no effect. The +
  button continues to start a new game.
- The clock operates only when the app is open.
- The app saves the game in progress (board, pencil marks, undo and redo history, clock). When the
  app opens again, the game is the same.
- The app saves each completed puzzle in a SQLite database. Each record has the time of completion,
  the difficulty (0–4), the quantity of starting numbers, the time used in seconds and the random seed
  of the puzzle generator. With the seed, the generator can make the same puzzle again.

**Menu**
- The close button shows only when the menu is open.
- There are five difficulty modes, from Easy to Hard. The new-game slider opens at the last mode
  that you selected. At the first start, it opens at the middle mode.
- "Start new game" replaces the current puzzle immediately.
- The difficulty meter below the board always shows the current puzzle. You cannot change it.

## Work to do

- **Puzzle generator (done).** `src/generator.rs` makes puzzles with only one solution. It sets the
  quantity of starting numbers for each mode (REQUIREMENTS.md §14.5).
- **Recognition pause (done).** The owner examined 0.5 s on the device (REQUIREMENTS.md §16).
- **E-ink check (done).** The owner examined the faint micro-cell lines, the pale cell colors and the
  gray icons on the tablet. All are clear.
- **Launcher (decided).** AppLoad (asivery/rm-appload), installed with Vellum. Refer to
  `REQUIREMENTS.md` §14.4.
- **Font.** reMarkable Sans is the font of reMarkable. The app must load it from the tablet
  (`/usr/share/remarkable/webui/reMarkableSans.woff2`). The app must not include a copy. Refer to
  `scripts/fonts/README.md`.
