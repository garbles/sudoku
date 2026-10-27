Communicate only in ASD-STE100 Simplified Technical English (STE). This applies to all messages, documents, comments and manual text. Use US spelling.

You are an AI agent. You build a Sudoku app for the reMarkable Paper Pro (https://developer.remarkable.com/documentation/sdk).

The Paper Pro has a color display of 1620x2160 pixels (portrait) at 229 DPI.

To build standalone executables on this machine, you must use the Linux @Dockerfile.

These documents tell how the app must operate:

- @REQUIREMENTS.md: all screens, states, inputs and saved values. It refers to the design images in `images/`. The images are exact. `scripts/build.sh` makes them, and `images/INDEX.md` gives a list of them.
- @INVARIANTS.md: the rules that must be true in all states that the app can get to. Use them as assertions and tests.

If the two documents do not agree, REQUIREMENTS.md is correct. When the behavior changes, update the two documents.

## Build & Test

All tests run on the computer. The Linux Dockerfile runs them again on aarch64 Linux and makes the
executable for the reMarkable Paper Pro.

```bash
cargo test                                   # all tests on the computer (macOS or Linux)
cargo run --release --bin snapshot -- DIR    # draw the reference states and compare them with images/
cargo run --release --bin video -- video/demo.txt video/demo.mp4   # demo video from an event script (ffmpeg)
docker build --platform linux/arm64 -f Dockerfile.imagegen --output type=local,dest=. .   # images/ and video/ with fixed tool versions (commit these)
docker build --platform linux/arm64 --output type=local,dest=dist .   # tests + static aarch64 build
```

`dist/sudoku/` is the AppLoad folder. Copy it to `/home/root/xovi/exthome/appload/` on the tablet.

## Architecture Overview

One Rust crate (`src/`). The app is an AppLoad external app with a qtfb framebuffer.

- `game.rs`: the board, the pencil marks, the undo and redo history, clashes and completion.
- `generator.rs`: the puzzle generator with a seed (only one solution). Version 1 stays for old saves.
- `solver.rs`: the human-style solver. It grades a puzzle by the most difficult technique it needs.
- `recognizer.rs`: reads handwriting in a cell ($P point-cloud recognizer with templates).
- `app.rs`: screens, menu, panels, overlays, pen and touch input, the clock. No input or output.
- `invariants.rs`: checks of the rules in INVARIANTS.md. Debug builds check them after each input.
- `layout.rs`, `render.rs`, `font.rs`: positions, drawing (tiny-skia) and the font of the tablet.
- `store.rs`, `settings.rs`: the SQLite results, the settings and the saved game (JSON).
- `platform/`: Linux only. The qtfb client, the eraser state (evdev) and the main loop.
- `reference.rs`: the states of the images in `images/`, for the snapshot tool and `tests/render.rs`.
- `bin/video.rs`: runs the app with a script of timed events (`video/*.txt`) and records a video.

## Conventions & Patterns

- Write all text, comments and documents in ASD-STE100 Simplified Technical English.
- Each behavior comes from REQUIREMENTS.md. Each state rule comes from INVARIANTS.md. Tests give the
  rule IDs that they examine.
- DECISIONS.md records how the app was examined (pixels, handwriting, device). If the code needs a
  detail that the requirements do not give, ask the owner before you add it to REQUIREMENTS.md or
  INVARIANTS.md.
- `sudoku --self-test OUT.png SECONDS` examines a tablet without AppLoad (font, drawing, recognizer,
  eraser).
- README.md must agree with the current state of the app (features, build, installation, data
  files). When a change makes README.md incorrect, update README.md in the same change.

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

