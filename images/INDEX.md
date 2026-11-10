# Image index

This document is written in ASD-STE100 Simplified Technical English (STE).

These images are the design reference for the sudoku app on the reMarkable Paper Pro. Each image is
1620×2160 (the portrait screen of the Paper Pro, 229 DPI), if the table does not give a different
size. The images are exact. The app must show the same pixels.

Scripts make all of the images, but not `icons.png`. Do not edit the images. To change an image,
change the scripts and run:

```sh
sh scripts/build.sh
```

The file `scripts/ui-lib.sh` has the layout values (board position, toolbar slots, fonts). The
dimensions of the toolbar, the panels and the fonts come from screenshots of a real Paper Pro
(`scripts/rmpp-screenshot.sh`).

All secondary text (gray labels, headings, help text, versions and the "?" of an unrecognized cell) is
`gray25` (#404040). No pixel of the app is `gray40`: the micro-cell lines show the gray of their setting (90 to 40) as a
pattern of `gray15` and white pixels (REQUIREMENTS.md §4.1), and the pencil marks are `gray15`. The only
exception is the anti-aliasing of the manual pages: an edge of a darker text or line there can have
single pixels of this value (REQUIREMENTS.md §15).

[`REQUIREMENTS.md`](../REQUIREMENTS.md) gives the full behavior of the app.

## Game screens

| Image | Content | Purpose |
|---|---|---|
| `screen.png` | Empty board, menu closed. It shows only the menu button (top right, closed icon: dot at the bottom left) and the difficulty meter below the board. | The base layout. The board has 9×9 cells of 147 px, at the center, from (148, 418) to (1471, 1741). The board lines (3 px and 13 px, black) are on full pixels, with no anti-aliasing. Each cell has a 3×3 grid of 49 px micro-cells: two vertical and two horizontal lines of 2 px, on the pixels 49, 50, 98 and 99 from the cell origin, over the full cell. Each pixel of a line is `gray15` or white, by an ordered 4×4 Bayer dither (`[[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]`) with the darkness of the setting: a pixel is `gray15` when `matrix * 16 + 8 <= 255 - gray` (gray90 = 230: 1 pixel of 8). The pattern starts at the first pixel of each line; its row index is the position along the line (modulo 4) and its column index is the position across the line (0 or 1). Thus all lines of the board have the same density, and all cells have the same pixels (`scripts/ui-lib.sh` `bayer_points`). The close button does not show when the menu is closed. |
| `screen-in-progress.png` | A game in progress: starting numbers (gray background, black digits), your numbers (white background, `#1E1E1E` digits), pencil marks and one clash (two red 9s). Box 0 at the top left is full, but its 9 clashes with a 9 outside the box. Thus the box has no completed color. | The main state of the game. The pencil marks are the possible digits only, because the app hides the marks of the digits of peers. The marks have no anti-aliasing: each pixel of a mark is `gray15` or the pixel under it. The board lines and the micro-cell lines also have no anti-aliasing. The other items (the numbers, the texts, the icons) keep the anti-aliasing of ImageMagick (REQUIREMENTS.md §15, §17). |
| `screen-highlight.png` | The same game after a tap on a 5. All 5s have the highlight color (Cyan). | Highlights. A tap on the same number again, or a different pen action on the board, removes the highlight. |
| `screen-unrecognized.png` | The same game immediately after a scribble that the app cannot read. A gray **?** (`gray25`) shows in the cell for approximately one second. | The response to writing that is not a number from 1 to 9 (for example a scribble, 0 or 12). After the "?", the cell is the same as before. |
| `screen-invalid-mark.png` | The same game immediately after a pen tap on the micro-cell of digit 6 in the empty cell at row 1, column 6 (from 0). A peer has a 6. A small gray **x** shows only in that micro-cell for approximately one second. The marks 3, 4 and 8 and the micro-cell lines of the cell stay. | The response to a pencil mark that a peer refuses, when "Hide pencil marks automatically" is on. The x has two lines with round ends, each from -10 px to +10 px on the two axes around the anchor point of the mark (20×20 px between the line centers), line width 3 px, `gray15`, no anti-aliasing (each pixel of the x is `gray15` or the pixel under it). After the x, the cell is the same as before. |
| `screen-completed-group.png` | A game with no clash. Box 0 (top left), column 4 and row 7 are complete. All their cells have the completed color (Green), also the cell where column 4 and row 7 cross, and the starting numbers in these groups. Other starting numbers stay gray. The state is the game of `screen-in-progress.png` without the 9 at row 5, column 1, and with the correct numbers in the cells (2,4), (6,4), (7,1), (7,2), (7,6) and (7,7) (row, column, from 0). | Completed groups. The color comes only from the current numbers. |
| `screen-completed.png` | The complete board with the normal colors: starting numbers on gray, your numbers on white. It has no completed color. The result card replaces the difficulty meter: a frame of 2 px at x 148–1471, y 1790–2110. The left half has "Completed in" (Regular 30, `gray25`), the time "23:41" (Medium 96), the difficulty slider at mode 2 (x 188–628) and "Average 18:55  ·  Previous best 14:51" (Regular 25, `gray25`). A `gray70` line at x 788 divides the card. The right half has "New game" and the buttons Easier, Same and Harder (white inside, black frame), each with a small meter of the mode that it starts (1, 2 and 3). | The complete, locked state, and the result card (REQUIREMENTS.md §8.3). The average and the best come from the results of `analytics.py` at mode 2, without the result of the current game (seed 1). No button is selected. A button that is not available (Easier at mode 0, Harder at mode 6) has a `gray80` frame, a `gray70` label and no meter. |
| `screen-completed-landscape.png` | `screen-completed.png` in landscape (2160×1620), left-handed, menu closed. The board is at x 418–1741, y 148–1471. The result card is a column at x 48–370, y 148–1471: "Completed in", the time (Medium 72), the meter (step 40 px), "Average at this level" 18:55 and "Previous best" 14:51, a `gray70` line at y 911, "New game" and the three buttons (each 120 px high, with the label and a small meter). | The card in landscape (REQUIREMENTS.md §3.3, §8.3). The card does not touch the toolbar: it is at the side opposite the toolbar. `scripts/toolbar.sh` makes it from `screen-completed.png`. |
| `screen-completed-landscape-right-handed.png` | The same state for a right-handed player. The menu button is at the top left (mirror icon), and the column is at x 1790–2112, with the same contents. | The card at the other side (REQUIREMENTS.md §3.3). `scripts/toolbar.sh` makes it. |

## Menu (open)

The menu is a column of 112 px on the right. It has the same design as the toolbar of the device.
From the top to the bottom, it has these buttons: the menu button (open icon: dot at the top left),
cell colors, settings (gear), undo, redo and checkpoints (bookmark). At the bottom, it has Help, Statistics and + (new game). When the menu is
open, the close button shows at the top left.

| Image | Content | Purpose |
|---|---|---|
| `screen-toolbar.png` | The menu is open on a new game. Undo and redo are gray, because there is no change to undo. | Shows all menu buttons and their icons. |
| `new-game-menu.png` | The + button is black, and its panel is open at x 1002–1507, y 1822 to the bottom edge. It has three rows of 112 px with a black line of 2 px between them (y 1934 and y 2048): the difficulty slider (seven modes, center y 1878), "Symmetrical" (Medium 25, x 1035, baseline y 2001) with a switch of the Settings panel at the right (on, right end at x 1474, center y 1992), and "Start new game" next to the +. | How to start a new game, and how to select a symmetrical puzzle (REQUIREMENTS.md §11). The switch shows the symmetry of the last new game; at the first start, it is on. The panel is wider than the menus of the device, so that the slider is easy to use. The slider has the same size as the meter below the board. |
| `color-menu.png` | The Cell colors panel is open on the game in progress, at x 708–1507 and y 112–1192. It has two columns of 400 px and three rows of 360 px. Left column, from the top: starting numbers (Gray), clashing numbers (Red) and highlighted numbers (Cyan). Right column, from the top: completed groups (Green), micro-cell lines (90) and empty space. A vertical line of 2 px at 400 px from the left of the panel is between the columns. In the Micro-cell lines section, each gray item shows the pattern of its gray inside its ring: a white disc of radius 17.5 px with the same Bayer dither as the board lines, which starts at the top left of the box of the disc (the pixel (cx - 19, cy - 19)), with the plain orientation of the matrix. None is a white dot. The selected item (90) keeps its black box and its white ring. | The only place to change the cell colors and the gray of the micro-cell lines. Each section has the same design as the pen-color menu of the device: a title, a 3×3 grid of cells (112 px wide, 90 px high) and a black box around the selected item. |
| `color-menu-right-handed.png` | The same state as `color-menu.png`, for a right-handed player. The toolbar is at the left edge (x 0–112), the menu icon is a mirror image (dot at the right), the close button is at the top right and the panel is at x 112–911, with the same order of columns. | The menu at the other side (REQUIREMENTS.md §3.1). The other icons and the contents of the panel do not change. `scripts/toolbar.sh` makes it from `color-menu.png`. |
| `color-menu-landscape.png` | The same state as `color-menu.png`, in landscape (2160×1620). The board is at x 418–1741, y 148–1471, the toolbar is at x 2048–2160, its bottom slot starts at y 1508 and the panel is at x 1248–2047, y 112–1192. | The panel fits in landscape (REQUIREMENTS.md §3.3, §10). `scripts/toolbar.sh` makes it from `color-menu.png` and `screen-in-progress.png`. |
| `color-menu-lines-90.png`, `color-menu-lines-40.png` | The same state as `color-menu.png`, with the micro-cell line gray 90 (the lightest) and 40 (the darkest) selected. The board below the panel has the pattern of that gray (gray40 = 102: 5 pixels of 8 are `gray15`). | The setting of the micro-cell lines (REQUIREMENTS.md §10, COL-7). `scripts/toolbar.sh` makes them. |
| `settings-menu.png` | The Settings panel is open on the game in progress (the gear is white on black). Three rows: "Highlight numbers", "Hide pencil marks automatically" and "Color completed groups" (two lines), each with a switch (all on). The fourth row is "Version" with the version of a development build ("dev", Regular, black) at the right. The fifth row is "Third-party licenses" with a › at the right (REQUIREMENTS.md §9.4). The panel stays 400 px wide. | The only place to turn the three settings off and on (REQUIREMENTS.md §9.1). |
| `screen-toolbar-update.png` | The menu is open on the game in progress of a stable build (v0.1.0). An update (v0.2.0) is available: the gear has the red caution icon at its top right. | The caution icon (REQUIREMENTS.md §9.2). |
| `settings-update.png` | The Settings panel of the stable build v0.1.0 when v0.2.0 is available. The Version row shows "v0.1.0". The sixth row has the button "Update to v0.2.0" with the update icon. The gear has the caution icon. | The update button (REQUIREMENTS.md §9.2). |
| `settings-up-to-date.png` | The Settings panel of the stable build v0.1.0 just after a check that found no update. The sixth row shows "Sudoku is up to date" (Regular, gray25). No caution icon. | The text states of the update row (REQUIREMENTS.md §9.2). The other texts have the same position and font. |
| `settings-check-updates.png` | The same panel after it closed and opened again. The sixth row has the button "Check for updates" with a circular arrow icon. | The check button (REQUIREMENTS.md §9.2). |
| `settings-restart.png` | The Settings panel of the stable build v0.1.0 after the update v0.2.0 is installed and replaced the ink patch, and the user tapped "Later". The sixth row has the button "Restart interface" with a power icon. The gear has the caution icon. | The restart button (REQUIREMENTS.md §9.2, UPD-10). The text "Restart Sudoku to update" (no patch change) has the same position and font as "Sudoku is up to date". |
| `update-restart.png` | `settings-restart.png` with the dialog "Restart the interface?" on the pale screen: two lines of text, "Later" (left) and "Restart" (white on black, right). | The restart dialog (REQUIREMENTS.md §9.2, UPD-10). Same design and position as `analytics-delete.png`. |
| `screen-completed-menu.png` | The menu is open on a complete puzzle. Undo, redo and the bookmark are gray (`gray70`). The result card stays below the board. | Shows that a complete puzzle is locked. The Checkpoints panel is not available. |
| `checkpoints-menu-empty.png` | The Checkpoints panel is open on the game in progress, with no checkpoint: x 908–1507, y 560–864. The bookmark is white on black. "Save checkpoint" is black. "No checkpoints" and one line of help text are below it. | The panel before the first save (REQUIREMENTS.md §9.3). |
| `checkpoints-menu.png` | The same panel with three rows (y 560–1212), the newest first: 18:32 and 40 cells, 11:07 and 36 cells, 4:15 and 32 cells. Each row has a small board, the clock, the quantity of cells, "Restore" and a bin. The board of a row has the clash color, the completed color, the starting color and a dot for each visible pencil mark. | A checkpoint row (REQUIREMENTS.md §9.3). The rows have the states `clean`, `mid` and `early` of `scripts/board-state.py`. |
| `checkpoints-menu-current.png` | The same panel with the maximum of five rows (y 560–1572). The first row has the same board as the game (20:14, 41 cells, one clash) and a black "Current" button. The second row (19:26, 46 cells) has three completed groups. "Save checkpoint" is gray (`gray70`). | The "Current" button, the save that is not available, and the maximum size of the panel (REQUIREMENTS.md §9.3). |
| `checkpoints-menu-right-handed.png` | `checkpoints-menu.png` for a right-handed player. The toolbar is at the left edge and the panel is at x 112–711, with the same order of items. | The panel at the other side (REQUIREMENTS.md §3.1). `scripts/toolbar.sh` makes it from `checkpoints-menu.png`. |
| `checkpoints-menu-landscape.png` | `checkpoints-menu.png` in landscape (2160×1620). The panel is at x 1448–2047, y 560–1212. | The panel in landscape (REQUIREMENTS.md §3.3). `scripts/toolbar.sh` makes it from `checkpoints-menu.png` and `screen-in-progress.png`. |

## Statistics

`scripts/analytics.py` also makes the four screens for landscape (`analytics-landscape.png`,
`analytics-games-landscape.png`, `analytics-delete-landscape.png`, `analytics-delete-all-landscape.png`, 2160×1620). The games list then is
one table across the full width, with 6 games on each page.

Statistics in the menu opens these screens (the image files keep the name "analytics"). They have the same top and bottom items as the manual:
"< Back" shows the game again from the summary, and the summary again from the list. The two screens
use one list of sample results. Thus their numbers agree.

| Image | Content | Purpose |
|---|---|---|
| `analytics.png` | The quantity of completed puzzles and the average time for all puzzles. Below these is one row for each difficulty (meter, quantity, average time) and the "All completed games" button. | The summary of the results in the SQLite database, with "All completed games" and "Delete all games". If a difficulty has no games, its average is "–". |
| `analytics-games.png` | All completed games, with the newest game first. Each row has the date and time of completion, the difficulty, the time used and a bin to delete the game. A filter row (All, or one difficulty) is above the list. Each page has 12 games (6 in landscape), with page arrows. A swipe also turns the page. | How to see and delete results. |
| `analytics-delete.png` | The list after a tap on a bin. A dialog shows "Delete this game?", the date and time of the game, and Cancel and Delete. | A deletion is permanent. Thus the app asks for a confirmation first. Delete removes the result, and the summary shows the change. |
| `analytics-delete-all.png` | The summary after a tap on "Delete all games". A dialog shows "Delete all games?", the quantity of games, and Cancel and Delete. | Delete all removes all results. The app asks for a confirmation first. |

## Licenses

`scripts/analytics.py` draws these screens from `assets/licenses.json`. When the file changes (a
change of `Cargo.lock`), run `make media-images`. Each image has a landscape version
(`licenses-landscape.png`, `licenses-text-landscape.png`, 2160×1620: 9 rows and 27 lines).

| Image | Content | Purpose |
|---|---|---|
| `licenses.png` | The first page of the Licenses list: "< Back", the title "Licenses", one line of gray text, then 14 rows. Each row has the name (Medium), the version (gray, at the right) and a ›. | The list of the third-party components (REQUIREMENTS.md §9.4). The rows do not show the license. |
| `licenses-text.png` | The first page of the license text of "UJI Pen Characters": the title, "Version 2 · CC-BY-4.0" (gray), the file name and the text, wrapped at 80 characters. | The license text of one entry (REQUIREMENTS.md §9.4). This entry does not change when the dependencies change. |

## Parts

| Image | Content |
|---|---|
| `template.png` | A blank canvas of 1620×2160. |
| `box.png` | One 3×3 box with 20 px of space around it, at the pixel scale of the board. |
| `cell.png` | One cell with its micro-cells, with 20 px of space around it, at the same scale. |
| `icons.png` | 1200×1050. All icons of the app, each with its name, in each of its looks (normal, white on the black slot, gray), then the AppLoad icon. The snapshot tool draws it with the code of the app (`reference::icon_sheet`), not the scripts: `make media-images` makes it. It is a list for people, not a design reference. |

## Manual

Each page also has a landscape version (`manual-<page>-landscape.png`, 2160×1620) with the same content
in two columns. `scripts/manual-<page>-landscape.sh` makes it.

Help in the menu opens the manual. Each page has "< Back" at the top left. It shows the game again.
The settings pages of the device have the same item. Each page also has arrows for the previous and
next page, next to the page number. The page number is gray25 (owner decision 2026-11-10).

| Page | Image | Content |
|---|---|---|
| 1 | `manual-how-to-play.png` | The rules, starting numbers and your numbers, the close button with the note that only the pen closes the game, and the menu button. |
| 2 | `manual-candidates.png` | Pencil marks: how to tap micro-cells to add and remove them. |
| 3 | `manual-pen.png` | How to write a number (the app reads it and shows a typed number) and how to erase it (the pencil marks show again). |
| 4 | `manual-pen-rules.png` | What a tap is. Finger touches have no effect. One number for each cell. Writing that starts on a number shows no ink. Writing that the app cannot read shows **?**. |
| 5 | `manual-helpers.png` | Automatic pencil mark hiding, highlights and completed groups: a full group with no clash gets the completed color, a clash with a cell outside the group stops the color, and the color is not a check against the solution. Settings can turn each one off. |
| 6 | `manual-mistakes.png` | Numbers in a clash get the clash color. How to correct them. The section "The app does not find all mistakes": the app does not compare the numbers with the solution, and a completed group can have an incorrect number. |
| 7 | `manual-menu.png` | All menu buttons (the gear: the helpers, the version and the licenses), and how to select a color in the five sections of the Cell colors panel. White in Completed groups gives no completed color. |
| 8 | `manual-new-game.png` | How to start a new game, step by step. The difficulty meter, which you cannot change. |
| 9 | `manual-finished.png` | How a puzzle becomes complete, the lock, the clock and the saved results. |

## Installer

The terminal output of `install.sh` (REQUIREMENTS.md §14.6), in the simulation of
`tests/install/run.sh`. For the version menus, `tests/install/tty.py` runs the scenario in a
terminal and pushes the arrow keys. `scripts/terminal.py` draws each image (width 1549 px, the height changes
with the output). The font is DejaVu Sans Mono (`scripts/fonts/`).

| Image | Scenario | Content |
|---|---|---|
| `install-first.png` | `first` | A new tablet in developer mode. All 11 steps operate: the SSH key and the password, Vellum, AppLoad, the hashtable, the ink patch, xovi, the power-button starter and Sudoku. |
| `install-connect.png` | `connect` | The tablet does not answer (instructions for the USB cable), then SSH is off (instructions for developer mode). All other steps are done already. |
| `install-restart.png` | `restart` | After a restart of the tablet with an earlier version of Sudoku: the installer starts xovi, the power-button starter is on already, and the installer replaces the app. |
| `install-again.png` | `again` | The installation steps (4 to 11) are done already. The installer changes nothing on the tablet. |
| `install-menu.png` | `select` (in a terminal) | The user selected "A different version". The list of versions is open: the newest first, the type (`latest`, `stable` or `preview`) and the date. The arrow is on the preview version `pre-1a2b3c4`. |
| `install-select.png` | `select` (in a terminal) | The complete run after the selection of `pre-1a2b3c4`: the menus changed to the line "Version: pre-1a2b3c4.", with the warning for a preview version. |

## Behavior summary

This section is a short summary. `REQUIREMENTS.md` gives the full behavior.

**Input**
- On the board, only the pen changes the numbers and the pencil marks. Finger touches remove the
  **?** and the x, not a highlight.
- In the manual and the games list, a finger swipe to the left or the right turns the page. The page
  arrows also turn the page. reMarkable notebooks operate in the same way.
- A tap is a pen touch that stays in one micro-cell. On an empty cell, a tap adds or removes the pencil
  mark of that micro-cell. On a number, a tap highlights all cells with that number.
- The app reads writing in an empty cell after a short pause (1 s) after the pen
  lifts. Thus a number with more than one stroke is correct.
- If the writing is a number from 1 to 9, the app removes the ink and shows a typed number. The pencil
  marks of that cell are hidden. They show again if you erase the number.
- If the writing is not a number from 1 to 9 (for example a scribble, 0 or 12), the app shows **?** for
  approximately one second. Then the cell is the same as before.
- Writing on a starting number or on your number has no effect. To change a number, erase it first.
- The eraser removes your number and shows the pencil marks of the cell again. It has no effect on
  starting numbers or empty cells.

**Helpers**
- When you write a number, the app hides that digit in the pencil marks in the same row, column and
  box. It keeps the marks: erase or undo the number, and they show again.
- A highlight gives the highlight color (default: Cyan) to all cells with the same number. A cell
  that clashes keeps the clash color. A tap on the same number again removes the highlight. A pen tap
  or stroke on the board also removes it. The hand, the menu and panels do not.
- There are four cell colors: starting numbers (default: Gray), clashing numbers (default: Red),
  highlighted numbers (default: Cyan) and completed groups (default: Green). You select each color
  from nine colors in the Cell colors panel: Gray `#AEAEAE`, White `#FFFFFF`, Yellow `#FFD740`, Blue
  `#408CFF`, Red `#FF8E8E`, Green `#48D630`, Cyan `#6FCFE6`, Magenta `#E650E0` and Orange `#FF9A1F`.
  Black is not available.
- The four colors must be different. White is the only exception: all four can be White. If a
  section uses a color, the other sections show a line through it.
- The background of a filled cell is the first rule that applies: the clash color, the highlight
  color, the completed color, the starting-number color, white.
- Numbers in a clash (the same digit in a row, column or box) get the clash color. The app does not
  compare the numbers with the solution.
- A row, a column or a box with nine filled cells and no clashing cell is complete. A clash with a
  cell outside the group also stops the group. All nine cells of a complete group get the completed
  color. This color does not tell that a number is correct.
- The switch "Color completed groups" off, or White in the Completed groups section, removes the
  completed color. A starting number then keeps its starting color.

**Puzzle completion**
- A puzzle is complete when all cells have a number and there is no clash.
- A complete puzzle shows no completed color: starting numbers have the starting color, and your
  numbers have a white background.
- The result card replaces the difficulty meter: "Completed in m:ss" (h:mm:ss after one hour), the
  mode, the average and the best time of the earlier games at this mode, and the buttons Easier,
  Same and Harder. One tap on a button starts a new game at that mode, with the menu closed.
- The board locks. Writing, the eraser, pencil marks, highlights, undo, redo and the bookmark have no effect. The +
  button continues to start a new game.
- The clock operates only when the app is open.
- The app saves the game in progress (board, pencil marks, undo and redo history, clock). When the
  app opens again, the game is the same.
- The app saves each completed puzzle in a SQLite database. Each record has the time of completion,
  the difficulty (0–6), the quantity of starting numbers, the time used in seconds, the random seed
  of the puzzle generator and the generator version. With the seed and the generator version, the
  generator can make the same puzzle again.

**Menu**
- The close button shows only when the menu is open.
- There are seven difficulty modes, from Easy to Hard. The new-game slider opens at the last mode
  that you selected. At the first start, it opens at mode 2.
- "Start new game" replaces the current puzzle immediately.
- The difficulty meter below the board shows the mode of the current puzzle. When the puzzle is
  complete, the result card replaces it. You cannot change the meter.
- The bookmark opens the Checkpoints panel. "Save checkpoint" keeps the numbers, the marks and the clock
  (at most 5 checkpoints). "Restore" puts them back as one new item of the undo history. The bin
  removes the checkpoint at once. A new game removes all checkpoints.

## Work to do

- **Puzzle generator (done).** `src/generator.rs` makes puzzles with only one solution. It sets the
  quantity of starting numbers for each mode (REQUIREMENTS.md §14.5).
- **Recognition pause (done).** The owner examined 1 s on the device (REQUIREMENTS.md §16).
- **E-ink check (done).** The owner examined the faint micro-cell lines, the cell colors and the
  gray icons on the tablet. All are clear.
- **Launcher (decided).** AppLoad (asivery/rm-appload), installed with Vellum. Refer to
  `REQUIREMENTS.md` §14.4.
- **Font.** reMarkable Sans is the font of reMarkable. The app must load it from the tablet
  (`/usr/share/remarkable/webui/reMarkableSans.woff2`). The app must not include a copy. Refer to
  `scripts/fonts/README.md`.
