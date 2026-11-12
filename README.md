# Sudoku for reMarkable Paper Pro and Paper Pro Move

> [!NOTE]
> The app has not been tested on a real Paper Pro Move. Only the design images and the tests on the
> computer examine the Paper Pro Move version. If you have a Paper Pro Move and you find a problem,
> please [open an issue](https://github.com/garbles/sudoku/issues/new).

A Sudoku game for the color e-ink display. It is the best way to play Sudoku: it feels like pen and
paper, and it does the work that paper cannot do.

<p align="center">
  <a href="video/demo.mp4"><img src="video/demo.gif" alt="Demo: pencil marks, numbers, a clash, the Cell colors menu and the eraser" /></a>
</p>

## Features

**Play with the pen**

- **Handwriting**: Write your numbers in the cells, as on paper.
- **Pencil marks**: Keep track of the possible digits in each cell.
- **Eraser**: Remove your numbers with the eraser end of the pen.

**Helpers**

- **Hide impossible pencil marks**: Pencil marks that are no longer possible are hidden.
- **Clashes**: See a repeated digit in a row, column or box immediately.
- **Highlight on tap**: Tap a number to highlight them all.
- **See completed groups**: Highlight your progress in each row, column and box.
- **Checkpoints**: Use the Nishio strategy without having to manually undo your failed attempt.

**Puzzles**

- **7 difficulty modes**: Play at your level, from Easy to Hard.
- **Toggleable symmetry**: Optionally play non-symmetrical puzzles.
- **One solution**: Each puzzle has a unique solution. No guessing.

**Other**

- **Statistics**: Follow your times and your completed games.
- **Portrait and landscape mode**: Hold your reMarkable in any way you like.
- **Autosave**: Game saves after every move.
- **Updates**: Install application updates with one tap.

## Installation

You need a computer with macOS or Linux and a reMarkable Paper Pro or Paper Pro Move in [developer mode](https://support.remarkable.com/s/article/Developer-mode).
Run [the step-by-step installer](https://github.com/garbles/sudoku/blob/main/install.sh) in your terminal:

```bash
curl -fsSL https://github.com/garbles/sudoku/releases/latest/download/install.sh | bash
```

The script can occasionally fail; however, the steps are idempotent so running it again does not have any adverse effect.

### Manual installation

If you would rather install this application manually:

1. Install the AppLoad launcher with Vellum:

   ```bash
     vellum add appload
     /home/root/xovi/rebuild_hashtable
   ```

2. Download `sudoku-<version>.tar.gz` and the ink patch `sudoku-penregion.qmd` from the
   [Releases](https://github.com/garbles/sudoku/releases) page. Extract the archive to `dist/`
   (`mkdir -p dist && tar -xzf sudoku-<version>.tar.gz -C dist`). Alternatively, you can [build it yourself](#build).

3. Copy the ink patch, write its stamp file (the version and the SHA-256 checksum of the patch),
   start xovi and install the power-button starter. The user interface of the tablet starts again:

   ```bash
     scp sudoku-penregion.qmd root@10.11.99.1:/home/root/xovi/exthome/qt-resource-rebuilder/
     ssh root@10.11.99.1 'cd /home/root/xovi/exthome/qt-resource-rebuilder && printf "{\"version\": \"%s\", \"sha256\": \"%s\"}\n" <version> "$(sha256sum sudoku-penregion.qmd | cut -d " " -f 1)" > sudoku-penregion.json'
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

4. Copy the `dist/sudoku` folder to the tablet and make the program executable:

   ```bash
     scp -r dist/sudoku root@10.11.99.1:/home/root/xovi/exthome/appload/
     ssh root@10.11.99.1 "chmod 755 /home/root/xovi/exthome/appload/sudoku/sudoku"
   ```

   `10.11.99.1` is the address of the tablet on a USB connection. On Wi-Fi, use the IP address of
   the tablet.

5. Open AppLoad on the tablet and tap **Sudoku**.

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

You can push development builds over SSH after running `rm-ssh-over-wlan on` on the tablet. Store the address at `REMARKABLE_HOST` in `.env`
and `make install-dev` will use it to push over wifi. To keep the address of your tablet, copy `.env.sample` to `.env` and set `REMARKABLE_HOST` (git ignores `.env`).

[`AGENTS.md`](AGENTS.md) gives the other tools (reference images, demo video, performance bench,
tablet self-test) and the release procedure.

## Developing

See the full list of commands by running:

```
make help
```

The source code is for the agents. It is pure slop, so you should probably not try to change it manually.

I'm not interested in contributions that:

- Refactor the code.
- Bump libraries.
- Add new features. Please [open an issue](https://github.com/garbles/sudoku/issues/new).

## License

MIT

The app contains open source components with their own licenses. `scripts/licenses.py` collects them
from `cargo metadata` into `assets/licenses.json`, and the app shows them (Settings > Third-party
licenses). After a change of `Cargo.lock`, run `python3 scripts/licenses.py` (a test fails if the file is
not up to date).
