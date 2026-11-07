# Sudoku for reMarkable Paper Pro

A Sudoku game for the color e-ink display. It is the best way to play Sudoku: it feels like pen and
paper, and it does the work that paper cannot do.

<p align="center">
  <a href="video/demo.mp4"><img src="video/demo.gif" alt="Demo: pencil marks, numbers, a clash, the Cell colors menu and the eraser" /></a>
</p>

## Features

**Play with the pen**

- Write a digit from 1 to 9 in a cell. The app reads your handwriting after a short pause.
- Tap a micro-cell to add or remove a pencil mark. A digit that the same row, column or box already
  has does not show. A tap on it does not change the game, and a small x shows in that micro-cell
  for a moment. If you turn the pencil mark hiding off, the tap adds or removes the mark, and no x
  shows.
- Rub one of your numbers with the eraser end of the pen to remove it.
- Your finger operates the menu and the panels. Only the pen operates the close button, thus your
  hand cannot close the app. A finger touch on the board does not add or remove
  a number or a pencil mark.

**Helpers**

- When you add a number, the app hides that digit in the pencil marks in the same row, column
  and box. If you erase the number, the marks show again.
- When the same digit is two times in a row, column or box, the two cells change color.
- Tap a number to highlight all cells with that digit. A cell that clashes keeps the clash color.
- When a row, column or box is full and none of its cells clashes, its nine cells change to the
  completed color (green by default). The app does not compare the numbers with the solution, thus
  this color does not tell you that the numbers are correct. A clash with a cell outside the row,
  column or box also stops the color. A completed game does not show this color.
- You can turn the pencil mark hiding, the highlights and the completed color off. You can also select the cell colors and the gray of the micro-cell lines (or no lines).
- Undo and redo for all changes on the board.
- Checkpoints: save the board (numbers and pencil marks) with the bookmark button, up to 5 at one
  time. Later, tap Restore to replace the board with a checkpoint. Undo reverses a restore. A new
  game removes the checkpoints.

**Puzzles**

- 7 difficulty modes, from Easy to Hard. Each mode has a level of solving techniques, from singles to chains.
- Each puzzle has only one solution, and you never have to guess.

**Other**

- A manual of 9 pages in the app.
- Statistics: your completed games, your times and the average time for each mode.
- Portrait and landscape layouts. For a right-handed player, the menu is on the left side.
- The app saves your game all the time. When you open the app again, you continue at the same point.
- The app uses the reMarkable Sans font of the tablet.
- The Settings panel shows the version of the app (`dev` or `dev/<name>` for a build on your computer).
- Updates from within the app.
- An installer that does all the installation steps for you.

## Installation

You need a reMarkable Paper Pro and a computer with macOS or Linux. Run this command in a terminal
of the computer:

```bash
curl -fsSL https://github.com/garbles/sudoku/releases/latest/download/install.sh | bash
```

The installer tells you what to do at each step:

1. It asks which version you want: the latest version (push Enter), or a different version from a
   list of all releases, for example a preview version. Then it downloads the release and its ink
   patch, and examines their checksums.
2. It finds the tablet on the USB cable. If developer mode is off, it tells you how to turn it on.

   **Developer mode erases all data on the tablet.** Sync your notebooks to the cloud first.

3. It makes an SSH key and adds it to the tablet. If the tablet does not accept another SSH key of
   your computer, you type the SSH password of the tablet one time. The installer tells you where to
   find it.

4. It installs [Vellum](https://github.com/vellum-dev/vellum) and the AppLoad launcher, makes the
   xovi hashtable, copies the ink patch and starts xovi. Then it installs the power-button
   starter: after a restart, press the power button 3 times to start xovi. The tablet must be
   connected to Wi-Fi for this step. With the ink patch, the ink has no gaps while you write.

5. It copies Sudoku to AppLoad.

The installer skips each step that is done. Thus you can run it again at all times:

- After an update of the tablet software. The installer updates Vellum and the xovi files, and it
  starts xovi again.
- To install a new version. (Sudoku also tells you about new versions in its Settings panel.)

After a restart of the tablet, xovi is off, and AppLoad is not in the menu. Press the power button
3 times to start xovi. The screen starts again after some seconds.

**If xovi is on, a triple press of the power button starts the interface again, and Sudoku closes.
Close Sudoku before you press the power button 3 times.**

To use Wi-Fi in place of the USB cable, set the address of the tablet:
`curl -fsSL .../install.sh | REMARKABLE_HOST=192.168.1.20 bash`, or
`export REMARKABLE_HOST=192.168.1.20` before the command. The variable must be in the environment of
`bash`: a variable that is not exported, or that is set only before `curl`, has no effect. If the
tablet does not answer on Wi-Fi, the installer asks if you want to try again or use the USB cable.

To select the version without the question, set its name from the
[Releases](https://github.com/garbles/sudoku/releases) page:
`curl -fsSL .../install.sh | SUDOKU_VERSION=pre-1a2b3c4 bash`.

### Manual installation

1. Install the AppLoad launcher with Vellum:

   ```bash
     vellum add appload
     /home/root/xovi/rebuild_hashtable
   ```

2. Download `sudoku-<version>.tar.gz` and the ink patch `sudoku-penregion.qmd` from the
   [Releases](https://github.com/garbles/sudoku/releases) page. Extract the archive to `dist/`
   (`mkdir -p dist && tar -xzf sudoku-<version>.tar.gz -C dist`). Or build the app (refer to
   [Build](#build)): then the ink patch is `dist/sudoku/sudoku-penregion.qmd`.

3. Copy the `dist/sudoku` folder to the tablet and make the program executable:

   ```bash
     scp -r dist/sudoku root@10.11.99.1:/home/root/xovi/exthome/appload/
     ssh root@10.11.99.1 "chmod 755 /home/root/xovi/exthome/appload/sudoku/sudoku"
   ```

   `10.11.99.1` is the address of the tablet on a USB connection. On Wi-Fi, use the IP address of
   the tablet.

4. Copy the ink patch, write its stamp file (the version and the SHA-256 checksum of the patch),
   start xovi and install the power-button starter. The user interface of the tablet starts again:

   ```bash
     scp sudoku-penregion.qmd root@10.11.99.1:/home/root/xovi/exthome/qt-resource-rebuilder/
     ssh root@10.11.99.1 'cd /home/root/xovi/exthome/qt-resource-rebuilder && printf "{\"version\": \"%s\", \"sha256\": \"%s\"}\n" v<version> "$(sha256sum sudoku-penregion.qmd | cut -d " " -f 1)" > sudoku-penregion.json'
     ssh root@10.11.99.1 /home/root/xovi/start
     ssh root@10.11.99.1 '/home/root/.vellum/bin/vellum update && /home/root/.vellum/bin/vellum add tripletap'
   ```

   An update in the app replaces the ink patch only when its checksum is different from the
   checksum in `sudoku-penregion.json`, or when this file is missing or not correct. Then the app
   asks you to start the interface again.

   After a restart of the tablet, press the power button 3 times to start xovi.

   After an update of the tablet software, run `/home/root/.vellum/bin/vellum upgrade`,
   `/home/root/.vellum/bin/vellum reenable`, `/home/root/xovi/rebuild_hashtable` and
   `/home/root/xovi/start`.

5. Open AppLoad on the tablet and tap **Sudoku**.

### Saved data

The app keeps its data in `/home/root/.local/share/rm-sudoku/` on the tablet:

| File             | Contents                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------- |
| `game.json`      | The current game: the puzzle, its seed, its mode and the version of the generator, your numbers and pencil marks, the undo and redo history, your checkpoints, the clock and the completion state |
| `settings.json`  | The cell colors, the micro-cell line gray, the last difficulty and the three switches of the Settings panel |
| `results.sqlite` | Your completed games: the date and time, the difficulty, the quantity of starting numbers, the time, the puzzle seed and the generator version |

To use a different folder, set the environment variable `SUDOKU_DATA_DIR`.

## Build

You need:

- The Rust toolchain, to run the tests on your computer (macOS or Linux).
- Docker, to build the program for the tablet.
- The reMarkable Sans font files `scripts/fonts/rmsans-400.ttf` and `scripts/fonts/rmsans-500.ttf`.
  reMarkable owns the font, so the repository does not contain these files. The tests and the
  Docker builds of the tests, the images and the videos need them. [`scripts/fonts/README.md`](scripts/fonts/README.md) tells how to make
  them from a tablet.

| Task                            | Command                                                               |
| ------------------------------- | --------------------------------------------------------------------- |
| Run all tests                   | `cargo test`                                                          |
| Run all tests on aarch64 Linux  | `docker build --platform linux/arm64 -f Dockerfile.test .`            |
| Build for the tablet (no tests) | `docker build --platform linux/arm64 --output type=local,dest=dist .` |

The Docker build makes the folder `dist/sudoku/`. It contains the program `sudoku`, the AppLoad
manifest `external.manifest.json`, the launcher icon `icon.png` and the ink patch
`sudoku-penregion.qmd`.

To keep the address of your tablet, copy `.env.sample` to `.env` and set `REMARKABLE_HOST` (git
ignores `.env`). Then load it into your shell before you run the installer or `ssh` and `scp`:

```bash
cp .env.sample .env          # then set REMARKABLE_HOST, for example 192.168.1.20
set -a; . ./.env; set +a     # REMARKABLE_HOST is now in the environment
./install.sh                 # the installer uses REMARKABLE_HOST
ssh root@"$REMARKABLE_HOST"
```

[`AGENTS.md`](AGENTS.md) gives the other tools (reference images, demo video, performance bench,
tablet self-test) and the release procedure.

## How the app was made

This app was started with **"Reverse Visual Regression Testing Driven Development"**. I had Claude generate screenshots of a non-existant application before any source code was written. A usual visual regression test compares the app with screenshots of an earlier version of the app. Here, the "screenshots" came first, before the app existed:

1. **Reference images.** Scripts in [`scripts/`](scripts/) draw exact images of each screen, panel and
   dialog at 1620×2160 in portrait, the statistics screens also at 2160×1620 in landscape, and the 9 pages
   of the manual in portrait and in landscape. The images are in [`images/`](images/), and
   [`images/INDEX.md`](images/INDEX.md) gives a list of them.

2. **Specification.** Two documents tell how the app must operate:
   - [`REQUIREMENTS.md`](REQUIREMENTS.md): all screens, sizes, inputs and saved data.
   - [`INVARIANTS.md`](INVARIANTS.md): the rules that must be true in all states of the app.
3. **Implementation and tests.** The app is written in Rust.
   - The render tests ([`tests/render.rs`](tests/render.rs)) compare the screens of the app with the
     reference images, pixel by pixel.
   - Debug builds examine the rules of `INVARIANTS.md` that apply to one state after each input.
     Each test gives the IDs of the rules that it examines.
   - Tests on the tablet examined the ink speed, the eraser and AppLoad
     ([`DECISIONS.md`](DECISIONS.md)).

## Contributing

Contributions are welcome, especially:

- Improvements to the puzzle generator ([`src/generator.rs`](src/generator.rs)), for example faster
  generation or better difficulty grades. Each puzzle must keep only one solution (rule `PUZ-2`).
- Corrections of bugs on the tablet or in the drawing.

[`AGENTS.md`](AGENTS.md) gives the rules for all changes, from people and from AI agents.

## License

MIT

---

The full source code of this project was generated with an LLM.
