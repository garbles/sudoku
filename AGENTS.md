Communicate only in ASD-STE100 Simplified Technical English (STE). This applies to all messages, documents, comments and manual text. Use US spelling.

You are an AI agent. You build a Sudoku app for the reMarkable Paper Pro (https://developer.remarkable.com/documentation/sdk).

The Paper Pro has a color display of 1620x2160 pixels (portrait) at 229 DPI.

To build standalone executables on this machine, you must use the Linux @Dockerfile.

These documents tell how the app must operate:

- @REQUIREMENTS.md: all screens, states, inputs and saved values. It refers to the design images in `images/`. The images are exact. `scripts/build.sh` makes them, and `images/INDEX.md` gives a list of them.
- @INVARIANTS.md: the rules that must be true in all states that the app can get to. Use them as assertions and tests.

If the two documents do not agree, REQUIREMENTS.md is correct. When the behavior changes, update the two documents.

## Build & Test

All tests run on the computer. `Dockerfile.test` runs them again on aarch64 Linux with musl, and the
`Dockerfile` makes the executable for the reMarkable Paper Pro. Each Docker build has one task:

| File                   | Task                                                                    |
| ---------------------- | ----------------------------------------------------------------------- |
| `Dockerfile`           | The static aarch64 build `dist/sudoku/`. No tests.                      |
| `Dockerfile.dev`       | The fast development build of `dist/sudoku/`. No tests. Not for release |
| `Dockerfile.test`      | All tests on aarch64 Linux: the stages `release` and `debug` (parallel) |
| `Dockerfile.images`    | The design images `images/*.png` (not `install-*.png`) and the icon     |
| `Dockerfile.installer` | The installer images `images/install-*.png`                             |
| `Dockerfile.video`     | The demo videos in `video/`                                             |

```bash
cargo test                                   # all tests on the computer (macOS or Linux)
cargo run --release --bin snapshot -- DIR    # draw the reference states and compare them with images/
cargo run --release --bin video -- video/demo.txt video/demo.mp4   # demo video from an event script (ffmpeg)
cargo run --release --bin video -- --no-gif video/demo-full.txt video/demo-full.mp4   # extended demo video for the website
scripts/bench.sh                             # performance bench: compare with bench/baseline-computer.json
docker build --platform linux/arm64 -f Dockerfile.test .   # all tests on aarch64 Linux (release and debug)
docker build --platform linux/arm64 --output type=local,dest=dist .   # static aarch64 build (no tests)
docker build --platform linux/arm64 -f Dockerfile.dev --output type=local,dest=dist .   # fast development build (no tests)
docker build --platform linux/arm64 -f Dockerfile.images --output type=local,dest=. .      # images/ and the icon (commit these)
docker build --platform linux/arm64 -f Dockerfile.installer --output type=local,dest=. .   # images/install-*.png (commit these)
docker build --platform linux/arm64 -f Dockerfile.video --output type=local,dest=. .       # video/ (commit these)
```

The `Makefile` gives a target for each of these commands, and more (`make help` shows them). If
`docker` is not on `PATH`, it uses the docker of OrbStack. `make test-linux` runs `Dockerfile.test`.
`make dist` runs `Dockerfile.test`, then builds `dist/`. `make install-dev` builds `dist/` with `Dockerfile.dev` and no
tests (thus run `cargo test` first) and installs it on the tablet at `REMARKABLE_HOST` (`.env`), or
on USB. `make install-dev FEATURES=corpus` (or `make install-corpus`) installs the corpus build
(the `remarkable-corpus` skill). `make media` makes the images, the icon and the videos (`make -j3 media` runs the three
builds in parallel).

`dist/sudoku/` is the AppLoad folder. Copy it to `/home/root/xovi/exthome/appload/` on the tablet.

## Performance

- `src/bench.rs` plays `video/demo-full.txt` with no screen, with the same output code as the device
  (`present.rs`). It measures each phase (`input_to_pixels`, `draw`, `blit`, `ink.*`, `handle`,
  `gen`, `recognize`, `startup.*` in the order of `platform::run`) and gives hashes of the output: the framebuffer at each check of
  the script, the update rectangles, the reference states, the puzzles and the recognizer matches.
  The bench plays the events with no real time between them. At a wake 100 ms or more after the
  last input (a timer or an input), it lets the background calculations (`early.rs`) finish (not
  measured), as on the device.
- `scripts/bench.sh` builds the bench, does one warm-up run and 5 runs, keeps the median, compares
  it with `bench/baseline-computer.json` and adds one line to `bench/history.jsonl`. `--save`
  writes a new baseline. `--note TEXT` describes the change in the history.
- `bench/baseline-linux.json` is the baseline in the Linux container (aarch64, musl, as the tablet
  build). The C library of the tablet is musl: its `memcmp` and `malloc` are much slower than on
  macOS. Thus the musl builds use mimalloc as the global allocator (`src/lib.rs`), and the code
  does not use `memcmp` for large comparisons. Also measure in the container: run
  `scripts/bench.sh --baseline bench/baseline-linux.json` in `rust:1.97-alpine` (with `python3`)
  and set `CARGO_TARGET_DIR` to a folder that is not `target/`. The recognizer hash on macOS is
  not the same as on Linux (the math functions of the C library are different): compare a result
  only with the baseline of the same platform.
- On the tablet: `sudoku --bench /tmp/bench.json`. Copy the file to the computer and compare it:
  `scripts/bench.sh --compare FILE --baseline bench/baseline-tablet.json`.
- Bench options: `--gen N` (seeds for each mode), `--recognize-step N` (each n-th example),
  `--draw-reps N`, `--script FILE`, `--dump DIR` (writes the framebuffer at each check and each
  reference state, to find the pixels that changed). `BENCH_STATE_TIMES=1` shows the time of each
  reference state.
- Rules for an optimization: change one thing at a time. Keep a change only if the time decreases by
  more than the noise (approximately 3%). The output hashes must not change: a change of a hash is a
  change of the pixels, the puzzles (PUZ-5) or the recognizer. `cargo test` must pass, and the
  snapshot tool must agree with `images/`. Then save the new baseline.
- A change of `video/demo-full.txt`, of the reference states or of the screens changes the hashes.
  Then save the baselines again (`--save`) in the same change.
- On macOS, `sample PID SECONDS` shows where the time goes. Build the bench with symbols:
  `CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_TARGET_DIR=target/prof cargo build --release --bin bench`.

## Release & CI

- Demo video: `video/demo.txt` is the event script. The video tool runs the real app with no screen
  and sends the pen, eraser and finger events of the script. Each run gives the same video.
  `src/bin/video.rs` gives the commands of the script. `video/demo.mp4` and `video/demo.gif` are for
  README.md.
- Extended demo video: `video/demo-full.txt` starts as `video/demo.txt` and plays the same puzzle to
  the completion, with pencil marks, pauses and incorrect numbers. `video/demo-full.mp4` (no GIF) is
  for the website. When you change the first part of `video/demo.txt`, change `video/demo-full.txt`
  also.
- `Dockerfile.images`, `Dockerfile.installer` and `Dockerfile.video` use fixed versions of
  ImageMagick and ffmpeg (Alpine 3.22), so each computer makes the same files. `scripts/build.sh`
  takes the parts `screens`, `installer` and `icon` (with no argument: all parts). The **Release**
  workflow (`release.yml`, job `media`, one job for each Dockerfile) makes `images/` and `video/`
  again in each run, and it fails when they are not the same as the files in git. When you change
  them, run `make media` and commit the result.
- Fonts in CI: git does not contain the reMarkable Sans files (`scripts/fonts/rmsans-*.ttf`). The
  secrets `SUDOKU_FONT_REGULAR` and `SUDOKU_FONT_MEDIUM` contain `rmsans-400.ttf` and
  `rmsans-500.ttf`, compressed with gzip and encoded with base64 (`gzip -9c F | base64`). The jobs
  `media` (images, video) and `test` of release.yml write the files before the Docker builds. Without the secrets, the run fails.
- One workflow, `release.yml` (**Release**), tests, builds and releases. These jobs run in
  parallel: `version` (the name of the release), `media` (three jobs: the images, the icon and the
  videos must agree with git), `test` (two jobs: the release tests and the debug tests on aarch64)
  and `build` (`dist/sudoku/` with the version, after `version`). Then `release` makes the GitHub
  release, only when all other jobs pass. The event selects the type of release.
- Prereleases (preview versions): each push to `main` runs the workflow. When all checks pass, it
  makes the prerelease `pre-<sha>`. A new push cancels the run of an older push.
- Stable releases: the owner starts the **Release** workflow manually (workflow_dispatch) on `main`. The release gets
  the name `v<version>`, with the version in `Cargo.toml`. Increase the version before each stable
  release. Use `cargo set-version --bump patch|minor|major` (from `cargo-edit`), not a manual edit.
  This command changes `Cargo.toml` and `Cargo.lock` together. Commit the two files. If the tag
  `v<version>` is already on GitHub, the run fails. To make the same release again, set the input
  `replace`. The run deletes the old release and its tag only after the new build passes all tests.
  A tablet that has the old build of this version does not update to the new build, because the
  version is not newer.
- Website: `site/index.html` is the GitHub Pages site (https://garbles.github.io/sudoku/).
  `pages.yml` publishes it on each push to `main` that changes `site/`, `video/demo-full.mp4` or
  the app icon. The workflow adds `demo-full.mp4` and `icon.svg` to the site. The instructions on the
  site must agree with README.md and `install.sh` (refer to Conventions & Patterns). In the repository settings, Pages > Source must be "GitHub Actions".
- App icon: `icon.svg` is the source. Its digits are outlines of reMarkable Sans Regular.
  AppLoad reads only `icon.png` and shows it at 150x150, thus `scripts/build.sh` (rsvg-convert)
  makes `appload/sudoku/icon.png` from the SVG. Do not edit the PNG. `Dockerfile.images` makes it,
  and the GitHub workflow compares it with the file in git.
- Each release has `install.sh` (REQUIREMENTS.md §14.6). release.yml replaces `@SUDOKU_VERSION@`,
  `@SUDOKU_SHA256@` and `@SUDOKU_PATCH_SHA256@` with the version, the checksum of the archive and
  the checksum of the ink patch. Thus the installer of a release installs that release. `tests/install/run.sh SCENARIO` runs the installer with fake `ssh`,
  `ssh-keygen` and `curl` (`tests/install/fake/`). The fake ssh finds each remote command from its
  first word (`: name;`). When you add or change a remote command in `install.sh`, change the fake
  ssh also.
- Each release has the ink patch `sudoku-penregion.qmd` as a separate file. release.yml moves it
  out of `dist/sudoku/`, thus the archive does not contain it. The installer and the update of the
  app download it, examine its checksum and write the stamp file `sudoku-penregion.json` next to
  it (REQUIREMENTS.md §14.4).
- Each release has `version.json`, with the version, the download URL of the archive and its
  SHA-256 checksum, and the download URL of the ink patch and its SHA-256 checksum:
  `{"version": "<version>", "url": "<url>", "sha256": "<checksum>", "patch_url": "<url>",
  "patch_sha256": "<checksum>"}` (the patch fields: owner approval 2026-11-01). Do not change
  this format without approval from the owner. The version is the name of the release (`v0.1.0`
  or `pre-<sha>`), and the archive is `sudoku-<version>.tar.gz`, for example
  `sudoku-v0.1.0.tar.gz`.
- Each release also has `version.json.sig`: the ed25519 signature (64 bytes) of `version.json`.
  release.yml makes it with the private key in the secret `SUDOKU_SIGNING_KEY` (PEM). The public
  key is `PUBLIC_KEY` in `src/update.rs`. A stable release fails without the secret. If the key
  changes, the builds with the old public key cannot update.

## Architecture Overview

One Rust crate (`src/`). The app is an AppLoad external app with a qtfb framebuffer.

- `game.rs`: the board, the pencil marks, the undo and redo history, clashes and completion.
- `generator.rs`: the puzzle generator with a seed (only one solution). Version 1 stays for old saves.
  After the first attempt, up to 4 threads make batches of attempts; the results are examined in
  the order of the attempts, thus the puzzle does not depend on the threads (PUZ-5). While the New
  game panel is open, the app draws the seed of the next game and makes its puzzle in the
  background (`generate_early`); Start new game then uses it.
- `solver.rs`: the human-style solver. It grades a puzzle by the most difficult technique it needs.
- `recognizer.rs`: reads handwriting in a cell ($P point-cloud recognizer with templates). Up to 4
  threads scan ranges of the examples; the main thread plays the accepted examples of all ranges
  again in order, thus the result is the same bit for bit. At each pen lift, the app starts the
  reading of the pending ink on a background thread (`read_early`). At the end of the pause,
  `recognize` uses that result if the strokes are the same (`early.rs`).
- `early.rs`: a calculation that starts on a background thread before the app needs its result
  (the reading of the ink at each pen lift, the puzzle of the next game). The result is a function
  of the key only, thus it is the same as a calculation at the time of need.
- `corpus.rs`: only in the corpus build (Cargo feature `corpus`, a development build). A blank
  board and a log of each stroke and erasure with its cell (`app::StrokeLog`), in a separate data
  folder. `examples/corpus.rs` makes a
  review image and adds the approved examples to `assets/owner-handwriting.jsonl`. The
  `remarkable-corpus` skill gives the procedure.
- `app.rs`: screens, menu, panels, overlays, pen and touch input, the clock. No input or output.
- `present.rs`: the output of the main loop: the drawing, the copy of the changes into the
  framebuffer and the ink updates. No input or output, thus the bench uses the same code. The
  frame of the presenter has a `render::FrameRecord`: a redraw of the game screen writes only the
  board tiles that changed and the areas of the other drawings, and the copy compares only the
  damaged rows. Each drawing on the frame must use `Canvas::retained` with the same record (debug
  builds examine it after each copy). In portrait, a large copy goes in bands of rows on up to 4
  threads. The rotation is a value of each thread: the code of a band must not use it.
- `invariants.rs`: checks of the rules in INVARIANTS.md. Debug builds check them after each input.
- `layout.rs`, `render.rs`, `font.rs`: positions, drawing (tiny-skia) and the font of the tablet.
  `render.rs` keeps the pixels of each board cell in a cache (tiles at their true position). Each
  `Canvas` operation records the area that it can change (for the `FrameRecord`). On a frame with
  a record, `put_overlay` also keeps the pixels of the meter and of the menu button with the
  toolbar (the key must contain all that the pixels depend on). The items of the Cell colors panel
  also have a cache (`put_swatch`): a tap in the panel draws only the items that changed. A full drawing of the board (the
  first frame, a new game) uses up to 4 threads, in stripes that end at the borders of cell rows
  (tiny-skia gives other pixels for a glyph that the edge of a pixmap cuts).
- `store.rs`, `settings.rs`: the SQLite results, the settings and the saved game (JSON). The main
  loop gives the JSON files to `store::Saver`, which writes them (with fsync) on a background
  thread; the completion, the restart of the interface and the close wait for it (`flush`).
- `update.rs`: the update state, the signed `version.json` and the installation of a release
  archive. No network: `platform/` downloads the files on a background thread (ureq, rustls).
- `platform/`: Linux only. The qtfb client, the eraser state (evdev) and the main loop.
- `reference.rs`: the states of the images in `images/`, for the snapshot tool and `tests/render.rs`.
- `appload/sudoku/sudoku-penregion.qmd` (not in `src/`): the ink patch, a QML patch of AppLoad that
  gives the cell of the ink the fast pen waveform (REQUIREMENTS.md §14.4). It uses names of xochitl
  and AppLoad: examine it after an update of the tablet software or of AppLoad (`remarkable-debug` skill).
- `install.sh` (not in `src/`): the installer for the tablet. It runs on the computer of the user.
  `scripts/terminal.py` draws its output (`images/install-*.png`).
- `script.rs`: the event scripts (`video/*.txt`) for the video tool and the bench.
- `bin/video.rs`: runs the app with a script of timed events (`video/*.txt`) and records a video.
- `bench.rs`, `bin/bench.rs`: the performance bench (refer to Performance).

## Conventions & Patterns

- Write all text, comments and documents in ASD-STE100 Simplified Technical English.
- Each behavior comes from REQUIREMENTS.md. Each state rule comes from INVARIANTS.md. Tests give the
  rule IDs that they examine.
- Each requested feature (a new behavior, or a change to what the app does or how it looks) uses
  the `remarkable-dev` skill (`.claude/skills/remarkable-dev/`). First make a prototype and install
  the dev build on the tablet. When the user approves the result, write a description of the
  change (no code), revert the prototype, and give the description to a team of agents. The team
  updates the documents in parallel (REQUIREMENTS.md, INVARIANTS.md, README.md and the other
  affected documents), then the source, then the tests and the media in parallel.
- DECISIONS.md records how the app was examined (pixels, handwriting, device). If the code needs a
  detail that the requirements do not give, ask the owner before you add it to REQUIREMENTS.md or
  INVARIANTS.md.
- `sudoku --self-test OUT.png SECONDS` examines a tablet without AppLoad (font, drawing, recognizer,
  eraser).
- The `remarkable-debug` skill (`.claude/skills/remarkable-debug/`) tells how to examine the app on
  the tablet: the procedure, the lessons about screen updates, and the techniques in `techniques/`
  (the test settings of the app, the screen update log of xochitl, the pen area, tests that show a
  screen problem). Read it before you change `present.rs`, the qtfb client or the ink patch. Add new
  lessons to `SKILL.md` and new techniques to `techniques/`.
- When the user asks you to debug something that does not operate correctly, first read the
  `remarkable-debug` skill and its `techniques/`, and use the procedures that are there. When the
  work is done, update the skill if you did something that it does not give yet: a new technique
  (add a file to `techniques/`), a new lesson or cause (add it to the lessons in `SKILL.md`), or a
  better step (change the step). Do not put xochitl internals (addresses, hashes) in the skill.
- README.md must agree with the current state of the app (features, build, installation, data
  files). When a change makes README.md incorrect, update README.md in the same change.
- README.md, `install.sh` and `site/` must always give the same instructions to the user: the
  install command, the installation steps, the requirements (tablet, computer, Wi-Fi, developer
  mode), the cautions and the environment variables. When you change the instructions in one of
  them, change the other two in the same change. Do not send a change where they do not agree.
- Run `cargo test` before you send a change.
- README.md is for players and contributors. Put maintainer and agent details (CI, releases,
  dev tools) in AGENTS.md.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

