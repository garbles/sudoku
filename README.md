# Sudoku for reMarkable Paper Pro

A Sudoku game for the color e-ink display of the reMarkable Paper Pro. You write the numbers with the
pen, and you add pencil marks with a tap.

<p align="center">
  <a href="video/demo.mp4"><img src="video/demo.gif" alt="Demo: pencil marks, numbers, a clash, the Cell colors menu and the eraser" width="48%" /></a>
</p>

## Features

**Play with the pen**

- Write a digit from 1 to 9 in a cell. The app reads your handwriting after a short pause.
- Tap a micro-cell to add or remove a pencil mark.
- Rub one of your numbers with the eraser end of the pen to remove it.
- Your finger operates the menu and the panels. Finger touches on the board have no effect.

**Helpers**

- When you add a number, the app removes that digit from the pencil marks in the same row, column
  and box.
- When the same digit is two times in a row, column or box, the two cells change color.
- Tap a number to highlight all cells with that digit.
- You can turn the pencil mark removal and the highlights off. You can also select the cell colors.
- Undo and redo for all changes on the board.

**Puzzles**

- 7 difficulty modes, from Easy to Hard. Each mode needs a different group of solving techniques.
- Each puzzle has only one solution, and you never have to guess.

**Other**

- A manual of 9 pages in the app.
- Statistics: your completed games, your times and the average time for each mode.
- Portrait and landscape layouts. For a right-handed player, the menu is on the left side.
- The app saves your game all the time. When you open the app again, you continue at the same point.
- The app uses the reMarkable Sans font of the tablet.

## Installation

You need a reMarkable Paper Pro with developer mode and SSH access.

1. Install the AppLoad launcher with Vellum, and start xovi:

   ```bash
   vellum add appload
   /home/root/xovi/rebuild_hashtable
   /home/root/xovi/start
   ```

   After a restart of the tablet, run `/home/root/xovi/start` again.

2. Build the app (refer to [Build](#build)).

3. Copy the `dist/sudoku` folder to the tablet and make the program executable:

   ```bash
   scp -r dist/sudoku root@10.11.99.1:/home/root/xovi/exthome/appload/
   ssh root@10.11.99.1 "chmod 755 /home/root/xovi/exthome/appload/sudoku/sudoku"
   ```

   `10.11.99.1` is the address of the tablet on a USB connection. On Wi-Fi, use the IP address of
   the tablet.

4. Open AppLoad on the tablet and tap **Sudoku**.

### Saved data

The app keeps its data in `/home/root/.local/share/rm-sudoku/` on the tablet:

| File | Contents |
|---|---|
| `game.json` | The current game: the puzzle, your numbers and pencil marks, the undo history and the clock |
| `settings.json` | The cell colors, the last difficulty and the two switches of the Settings panel |
| `results.sqlite` | Your completed games: date, difficulty, time and puzzle seed |

To use a different folder, set the environment variable `SUDOKU_DATA_DIR`.

## Build

You need:

- The Rust toolchain, to run the tests on your computer (macOS or Linux).
- Docker, to build the program for the tablet.
- ffmpeg, only to make the demo video.
- ImageMagick 7 and Python 3, only to make the reference images again.

| Task | Command |
|---|---|
| Run all tests | `cargo test` |
| Build for the tablet (also runs all tests on Linux) | `docker build --platform linux/arm64 --output type=local,dest=dist .` |
| Compare the app with the reference images | `cargo run --release --bin snapshot -- OUT_DIR` |
| Make the reference images again | `sh scripts/build.sh` |
| Make the demo video and GIF | `cargo run --release --bin video -- video/demo.txt video/demo.mp4` |
| Examine a tablet without AppLoad (run it on the tablet) | `sudoku --self-test OUT.png SECONDS` |

The Docker build makes the folder `dist/sudoku/`. It contains the program `sudoku`, the AppLoad
manifest `external.manifest.json` and the launcher icon `icon.png`.

GitHub Actions does the Docker build on each push to `main`
([`.github/workflows/build.yml`](.github/workflows/build.yml)). When all tests pass, you can download
the `dist/sudoku/` folder from the run as the artifact `sudoku`.

The demo video comes from the script [`video/demo.txt`](video/demo.txt). The video tool runs the real
app with no screen and sends the pen, eraser and finger events of the script. Each run gives the same
video. [`src/bin/video.rs`](src/bin/video.rs) gives the commands of the script.

## How the app was made

The app was made with **Reverse Visual Regression Testing Driven Development**. A usual visual
regression test compares the app with screenshots of an earlier version of the app. Here, the
"screenshots" came first, before the app existed:

1. **Reference images.** Scripts in [`scripts/`](scripts/) draw exact 1620×2160 images of each screen,
   panel and dialog, and the 9 pages of the manual in portrait and in landscape. The images are in
   [`images/`](images/), and [`images/INDEX.md`](images/INDEX.md) gives a list of them.
2. **Specification.** Two documents tell how the app must operate:
   - [`REQUIREMENTS.md`](REQUIREMENTS.md): all screens, sizes, inputs and saved data.
   - [`INVARIANTS.md`](INVARIANTS.md): the rules that must be true in all states of the app.
3. **Implementation and tests.** The app is written in Rust.
   - The render tests ([`tests/render.rs`](tests/render.rs)) compare the screens of the app with the
     reference images, pixel by pixel.
   - Debug builds examine all rules of `INVARIANTS.md` after each input. Each test gives the IDs of
     the rules that it examines.
   - Tests on the tablet examined the ink speed, the eraser and AppLoad
     ([`DECISIONS.md`](DECISIONS.md)).

## Contributing

Contributions are welcome, especially:

- Improvements to the puzzle generator ([`src/generator.rs`](src/generator.rs)), for example faster
  generation or better difficulty grades. Each puzzle must keep only one solution (rule `PUZ-2`).
- Corrections of bugs on the tablet or in the drawing.

Rules:

1. Write all text, comments and messages in ASD-STE100 Simplified Technical English. Use US spelling.
2. Make sure that your change agrees with `REQUIREMENTS.md` and `INVARIANTS.md`. If the behavior
   changes, update the two documents.
3. Run `cargo test` before you send the change.

## License

MIT

---

The full source code of this project was generated with an LLM.
