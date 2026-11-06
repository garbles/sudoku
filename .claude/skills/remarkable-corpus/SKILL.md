---
name: remarkable-corpus
description: Collect examples of the handwriting of the user on the reMarkable Paper Pro and add them to the examples of the recognizer (assets/owner-handwriting.jsonl). It asks which digits to collect, installs the corpus build (a blank board that keeps a log of each stroke with its cell), and collects the digits one set at a time (one blank board for each digit). Then it makes a numbered review image for each digit, asks which examples to exclude, and adds the other examples. Use when the user asks to add handwriting, examples, samples or training data to the recognizer corpus, or when the recognizer does not read a digit of the user.
---

# Add handwriting to the recognizer corpus

Write all messages and files in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

The recognizer (`src/recognizer.rs`) compares the ink with examples. The examples of the user are
in `assets/owner-handwriting.jsonl` (one JSON line for each example: `class` and `strokes` in the
box 0..255). This skill first collects all of the selected digits, one set at a time. Then it
makes the review images, and the user reviews them.

## The parts

- **The corpus build**: the Cargo feature `corpus` (`src/corpus.rs`). The source uses
  `#[cfg(feature = "corpus")]` only at two places: `store::data_dir` and the call
  `corpus::setup` in `platform::run`. The build has these differences:
  - It is a separate app: `Sudoku (corpus)` in AppLoad, in the folder `sudoku-corpus`. The usual
    Sudoku app stays on the tablet and does not change.
  - It has its own ink patch, `appload/sudoku-corpus/sudoku-corpus-penregion.qmd`, a copy of the
    usual patch for its name. When you change the usual patch, change the copy also. The app also needs the usual patch on the tablet. A release
    and `install.sh` do not use the copy.
  - The board is blank (no starting numbers), the menu is closed, and clashes, the highlight and the
    completed color do not show.
  - The data folder is `/home/root/.local/share/rm-sudoku-corpus`. Thus the build does not change
    the game, the settings or the results of the player.
  - Each pen stroke and each erasure on the board goes to `corpus-ink.jsonl` in that folder, with
    its cell (screen pixels). The log has all strokes, also a stroke that the app discards on a
    filled cell. The tool makes one example from all strokes of a cell, until the eraser touches
    that cell. Thus a digit with two strokes (a 4, an 8) is one example, also when the app reads
    the ink between the two strokes.
- **Make targets**:
  - `make install-corpus`: build and install the corpus build. The usual Sudoku app does not change.
  - `make corpus-start`: delete the corpus board and the corpus log on the tablet, and move the sets of earlier sessions to `target/corpus/old/`.
  - `make corpus-pull DIGIT=n`: copy the corpus log to `target/corpus/corpus-ink-n.jsonl`, then
    delete the corpus log and the corpus board on the tablet. Thus the next set starts on a blank
    board with an empty log. It fails (and deletes nothing) if the log is empty, if it cannot read
    the log, or if `target/corpus/corpus-ink-n.jsonl` exists already.
- **The tool** `examples/corpus.rs`:
  - `cargo run --release --example corpus -- sheet LOG OUT.png DIGIT`: the review image.
  - `cargo run --release --example corpus -- add LOG DIGIT [--exclude 3,7,12]`: add the examples.
  - `cargo run --release --example corpus -- count`: the quantity of examples of each digit.

## Procedure

The procedure has three phases: prepare, collect (one set for each digit) and review. Do not make
images or ask for a review before all sets are collected.

### Phase 1: prepare

1. **Find the tablet.** Use the `remarkable-host` skill (or USB, `10.11.99.1`) as in step 1 of the
   `remarkable-install-dev` skill. The Make targets use `REMARKABLE_HOST` of `.env`.

2. **Get the digits.** Ask the user which digits to write (AskUserQuestion, `multiSelect: true`).
   A question has a maximum of 4 options, thus send three questions in one call: "1 to 3",
   "4 to 6" and "7 to 9", each with its three digits as options. The user can select no digit in a
   question. Accept digits that the user types in "Other". If the user gave the digits in the request, do not ask. Keep the digits
   in increasing order. Run `count` and tell the user the current quantity of examples of each
   selected digit.

3. **Install the corpus build.** Run `make install-corpus` (timeout 600000 ms). It uses
   `Dockerfile.corpus`, with the version name `dev/corpus`. If `docker` is not on `PATH`, the Makefile uses
   OrbStack. If the build fails, show the error and stop. If the target tells that the ink patch
   changed, ask the user to close all notebooks, then run `make xovi-restart`. Without this, the ink of the app has
   gaps. If AppLoad does not show `Sudoku (corpus)`, tell the user to use the reload button of AppLoad.

4. **Start a clean session.** Tell the user to close Sudoku (corpus) on the tablet (the open app keeps the
   old program, and it saves the board when it closes). When the user confirms, run
   `make corpus-start`. This deletes the corpus board and the old corpus log on the tablet. The game of the
   player does not change. It also moves the sets
   and images of earlier sessions (`target/corpus/corpus-ink-*.jsonl`, `sheet-*.png`) to
   `target/corpus/old/`. Do not use them: each run of this skill collects new data only.

### Phase 2: collect, one set for each digit

Do steps 5 to 7 for each selected digit, in order. In these steps, DIGIT is the current digit.

5. **Ask the user to write.** Tell the user (one message):
   - Set N of M: the digit DIGIT.
   - Open Sudoku (corpus) from AppLoad. The board is blank.
   - Write the digit DIGIT in as many cells as possible, one digit in each cell, with the pen.
     Write it in the usual manner, with some variation of size and slant.
   - Write only one digit in each cell. All strokes in a cell are one example, also when the app
     shows a number or a "?" before you write the next stroke: then write the next stroke in the
     same cell. You can exclude bad examples in the review at the end.
   - Do not erase, except to write more than 81 examples: then erase the cells with the eraser
     and write again. An erasure ends the example of that cell.
   - When you are done, close Sudoku (corpus) and tell me.

   Wait for the answer of the user (AskUserQuestion with the options "Done", "Skip this digit"
   and "Stop"). "Skip this digit": go to the next digit (the log of this set stays on the tablet,
   thus run `make corpus-pull DIGIT=DIGIT` and move the result to `target/corpus/old/`, so that
   the next set starts clean). "Stop": go to phase 3 with the sets that you have.

6. **Get the set.** Make sure that the user closed Sudoku (corpus). Then run:

   ```bash
   make corpus-pull DIGIT=DIGIT
   ```

   It copies the log to `target/corpus/corpus-ink-DIGIT.jsonl` and clears the log and the board on
   the tablet. If it tells that the log is empty, ask the user to open Sudoku (corpus) and write the digit,
   and do step 5 again. The target tells the quantity of strokes. Do not make the image now:
   phase 3 gives the quantity of examples.

7. **Next set.** If there is a next digit, go to step 5 with that digit. The user opens Sudoku (corpus)
   again: the board is blank.

### Phase 3: review and add

8. **Make the review images.** For each collected digit:

   ```bash
   cargo run --release --example corpus -- sheet target/corpus/corpus-ink-DIGIT.jsonl target/corpus/sheet-DIGIT.png DIGIT
   ```

   The tool needs the fonts `scripts/fonts/rmsans-*.ttf` (refer to `scripts/fonts/README.md`).
   Read each image yourself (Read tool). Each tile has the number of the example (from 1), the
   cell, the quantity of strokes (`2s`) and the result of the recognizer. A red frame shows that the recognizer did not read DIGIT.
   Note the examples that you think are bad: an ink that is not DIGIT, an incomplete ink, a stroke
   of the next cell, two digits in one tile. A red frame alone is not a reason to exclude: these
   examples are the most useful.

9. **Ask for the exclusions.** Open all images on macOS (`open target/corpus/sheet-*.png`) and
    give their absolute paths. Then ask about one digit at a time: for each digit, give the
    quantity of examples, the red frames and your suggested exclusions, and ask which examples to
    exclude (the numbers of the tiles, for example `3, 7, 12`, or "none"). Do not exclude an
    example without the approval of the user.

10. **Add the examples.** For each digit, after its answer:

    ```bash
    cargo run --release --example corpus -- add target/corpus/corpus-ink-DIGIT.jsonl DIGIT --exclude 3,7,12
    ```

    Omit `--exclude` if there are no exclusions. Run `add` only one time for each log: a second
    run adds the same examples again. Then move the log out of the way, so that a new session does
    not use it again:
    `mkdir -p target/corpus/done && mv target/corpus/corpus-ink-DIGIT.jsonl target/corpus/done/corpus-ink-DIGIT-$(date +%Y%m%d-%H%M%S).jsonl`.

11. **Examine the result** (one time, after all digits are added).
    - Run `cargo test`. The test `recognizer::tests::owner_examples_are_correct` examines the file,
      and `writer_independent_accuracy` must still pass. If a test fails, show the result to the
      user and ask what to do (for example: remove the new lines of the file).
    - Make each sheet again from the archived logs (step 8 command). Now most tiles must have a
      green frame. Tell the user, for each digit, the quantity of tiles that changed from red to
      green.
    - The recognizer hash of the bench changes. Run `scripts/bench.sh --save --note "corpus: DIGITS"`
      and the Linux baseline (`make bench-linux-save`), as AGENTS.md "Performance" tells.

12. **End.**
    - Tell the user to close Sudoku (corpus). The app stays on the tablet for the next session.
      The usual Sudoku app did not change, thus it does not have the new examples until the next
      install (`make install-dev`, the `remarkable-install-dev` skill).
    - Report: for each digit, the quantity of examples that you added and the excluded numbers;
      the test result; and `count`.

Do not commit or push. Commit `assets/owner-handwriting.jsonl` and the bench baselines only when
the user asks.

## Notes

- The examples of the user have the writer 0 (as the typeface examples). The test
  `writer_independent_accuracy` uses them in the training part, and the writers 41 to 60 of the
  database as the test part.
- Do not add examples to `assets/handwriting.bin`: that file is the UJI database
  (`assets/README.md`).
- The log on the tablet has screen pixels. `add` scales each example so that the larger side is
  0..255 (the same box as the database).
