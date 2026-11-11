---
name: remarkable-colors
description: Copy the colors that the owner selected in the Sudoku (colors) app on the reMarkable Paper Pro into the usual Sudoku app. It pulls colors.json from the tablet, compares it with the source, gets the approval of the owner on the tablet, then changes the source, each copy of the values, the documents, the tests, the images, the videos and the bench. Use when the user asks to pull, apply, use, sync or copy the colors from the tablet or from the color picker, to update the app with the new colors, or says that the colors are ready, also if the user does not name colors.json.
---

# Copy the colors of the color picker into the app

Write all messages and files in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

`Sudoku (colors)` keeps its colors on the tablet in
`/home/root/.local/share/rm-sudoku-colors/colors.json`:

```json
{"palette": [[174,174,174], "... 9 colors, in the order of Color::ALL ..."], "number": [46,46,46]}
```

The usual app does not read this file, and a release must not depend on it. This skill makes the
values part of the source. The source of the values is `Color::rgb` and `ENTRY_DIGIT` in
`src/settings.rs`. The color picker uses the same values for its first colors and its reset, thus it
needs no change.

This skill keeps the main rule of the `remarkable-dev` skill: the owner must see the change on the
tablet before the documents and the tests change. A color in the picker looks different next to a
full board, the panels and the manual. The owner selected the values, thus there is no team of
agents and no revert: do the work directly. Do not commit or push unless the user asks.

## Procedure

**1. Prepare.** Make a beads issue (`bd create --type=task`, then `bd update <id> --claim`) with the
request of the user. If the working tree is not clean, tell the user which files changed, and do not
touch these changes.

**2. Pull and compare.**

```bash
make colors-pull
python3 .claude/skills/remarkable-colors/scripts/compare.py
```

- `make colors-pull` fails if the tablet does not answer: use the `remarkable-host` skill. It also
  fails if the file does not exist: the app writes the file only after the first change of a color.
- Exit status 2 from `compare.py` means that the colors are the same as the source. Tell the user,
  close the issue and stop.

**3. Ask the owner.** Show the table of `compare.py`. Then ask (AskUserQuestion) which colors to
apply: all the changed colors, or a part of them. Also ask about each caution that applies, because
the answer changes what the player sees:

| Condition | Why it is a decision |
| --- | --- |
| A color no longer agrees with its name | The panel and the manual show the name (for example "Yellow" with a red value). Ask if the name stays. |
| Two palette colors have the same value | Two sections of the Cell colors panel must have different colors (COL-5), and the player cannot see a difference between the two items. |
| White changes | `Color::White` is the background of a filled cell with no other color, and the cells around it stay white. The scripts use the name `white`. |
| Your number is not a gray | BRD-8 and the scripts give a gray. `tests/inv_render.rs` has the value as one gray level, thus the test must change. |

Default colors of the sections also come from the palette: a new value of Gray, Red, Cyan or Green
changes the default look of the game. Tell the owner.

**4. Change the source and install.** Change only `Color::rgb` and `ENTRY_DIGIT` in
`src/settings.rs`. Do not change the documents, the tests or `images/` yet. Then:

```bash
make install-dev DEV_NAME=colors
```

Tell the owner to close Sudoku and open it again from AppLoad, and to examine a game with starting
numbers, a clash, a highlight and a completed group, and the Cell colors panel. Then ask
(AskUserQuestion): "Correct", "Change it" or "Stop and revert". For "Change it", the owner can select
a new value in the picker (go to step 2) or give a value. Record each rejected value and the reason
in the notes of the issue. For "Stop and revert", restore `src/settings.rs`, close the issue with a
reason and stop.

**5. Change each copy of the values.** Do this only after "Correct". Change these places:

| Place | What it contains |
| --- | --- |
| `scripts/ui-lib.sh` | `COLOR_*`: the palette of the design images |
| `scripts/manual-lib.sh` | `GIVEN_BG`, `ENTRY_FG`, `CLASH_BG`, `COMPLETED_BG` and the other manual colors |
| `scripts/board-state.py` | `STARTING_BG`, `CLASH_BG`, `HIGHLIGHT_BG`, `COMPLETED_BG`, `PLAYER_DIGIT` |
| `tests/rules.rs`, `tests/behavior.rs`, `tests/inv_groups/mod.rs`, `tests/inv_render.rs`, `tests/render.rs` | Expected values. They do not come from the source, so that a test can find an error. |
| `REQUIREMENTS.md` | §4.1 and §15 (Numbers, Palette, Default colors), with the owner approval and the date |
| `INVARIANTS.md` | BRD-8 if the color of the numbers changes |
| `images/INDEX.md` | The list of the nine colors |
| Comments | `src/settings.rs` (`ENTRY_DIGIT`), `scripts/ui-lib.sh`, and the tests that give `gray18` or a hex value |

This list can become old. For each old value, search in all forms, and change each result except the
history in `DECISIONS.md` and `bench/`:

```bash
git grep -nIiE 'RRGGBB|0xRR, 0xGG, 0xBB|\[R, G, B\]' -- ':!DECISIONS.md' ':!bench' ':!.claude/skills/remarkable-colors'
```

Do not use `\b` in this pattern: `git grep -E` on macOS does not match it. When the color of the
numbers changes, also search for its name (for example `gray18`) and for its one-byte value
(`0x2E`), because a test can keep one gray level. Add a new place to this table.

Record in `DECISIONS.md`: the old and the new values, that the owner selected them in `Sudoku
(colors)` and examined them in a game on the tablet, and the rejected values.

**6. Make the media again.** The colors show in the images, the manual pages and the videos:

```bash
make -j3 media
```

Read a crop of `images/pro/screen-completed-group.png` and of `images/pro/color-menu.png`
(`magick IMAGE -crop WxH+X+Y +repage SCRATCH/crop.png`). They must show the new colors. The app
contains the manual pages (`include_bytes!`), thus the binary changes after this step.

**7. Examine and save the bench.**

```bash
cargo build --release --bins
cargo test
make snapshot
scripts/bench.sh --note "Colors from Sudoku (colors)"
```

The build must have no warnings, the tests must pass, and the snapshot must agree with `images/pro/`.
The bench hashes of the pixels change because of this change. No hash of the puzzles or of the
recognizer may change. Then save the baselines, as AGENTS.md "Performance" tells, for the computer
and for the Linux container (`scripts/bench.sh --save`, `make bench-linux-save`).

**8. Examine the final build.** Install the final dev build (step 4), because the manual pages
changed. Ask the owner to examine it one time.

**9. Close and report.** Close the issue. Give the old and new values, the changed files, the test
results, and the suggested commit commands. The file on the tablet does not change. After this
change, the picker starts from its saved colors, and "Reset colors" goes to the new values.
