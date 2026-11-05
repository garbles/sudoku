---
name: remarkable-dev
description: Develop each requested feature or change of behavior of the Sudoku app (the UI, the game rules, the input, the helpers, the puzzles, the recognizer, the saved data, the update, the installer). First make a fast prototype, install the dev build on the tablet and get the approval of the user. For a simple request, keep the approved code and finish the documents, the tests, the images and the bench directly (the fast track, 5 steps). For a complex request, a dev-prototype-reviewer agent reviews the approved prototype. Then write a description of the change, revert the prototype, and give the description to a team of agents in three waves - the documents (REQUIREMENTS.md, INVARIANTS.md, README.md and the other affected documents) in parallel, then the source, then the tests, the images and the other media in parallel. Use when the user asks for a new feature, or for a change to what the app does or how it looks.
---

# Develop a feature

Write all messages and files in ASD-STE100 Simplified Technical English (refer to AGENTS.md).

The user must try a feature on the tablet before the documents and the tests change. A change
that looks correct in the code can look or feel wrong on the e-paper display, with the pen. Thus
you first make a prototype and install it on the tablet. That part is the same for all requests.

Then this skill has two tracks:

1. **Fast track (simple request).** Keep the approved prototype as the final code. You change the
   documents, the tests, the images and the bench directly. There is no review, no description,
   no revert and no team. This track has 5 steps.
2. **Full track (complex request, the default).** A `dev-prototype-reviewer` agent reviews only
   the approved prototype, and its findings go into the description for the team. You write a
   description of the approved change, revert the prototype, and give the description to a team
   of agents. The team does the work in the usual order of AGENTS.md, in three waves: first the
   documents (3 agents in parallel), then the source (1 agent), then the tests and the media
   (N agents in parallel).

The prototype is only a tool to find the correct result. On the full track, its code does not go
into the final change. On the fast track, its code becomes the final code only after you examine
it with the checklist in "Fast track" below.

## When to use this skill

Use this skill for each feature that the user asks for, and for each change to what the app does or
how it looks: the UI, the game rules, the pen and touch input, the helpers, the puzzles, the
recognizer, the saved data, the update and the installer.

Use a different skill for these tasks:

- A problem on the tablet screen, or a bug: `remarkable-debug`.
- A refactor with no change of behavior: `improve-architecture`.
- An audit of the documents: `audit-doc-claims`.

## Choose the track

Choose the track before you change the code. Use the fast track only when all these items are true:

1. **One behavior, one place.** One sentence describes the result. One short action on the tablet
   shows it. There is no new screen, panel, dialog, button, icon, image file, manual page, input
   gesture or key.
2. **No new state.** There is no new state field, saved value, setting, file-format field, game
   rule or state transition. An existing value can change: the text, a color, a position, a size,
   a time, a limit or a default. §0 of INVARIANTS.md stays the same.
3. **Small code.** At most two files in `src/` change. You add no file in `src/`. You do not touch
   `generator.rs`, `solver.rs`, `recognizer.rs`, `update.rs`, `install.sh`, `appload/` or the
   corpus feature. `src/reference.rs` (the reference states of the snapshot tool) does not count
   as one of the two files. You can examine the diff in one pass.
4. **Small work.** Besides REQUIREMENTS.md, DECISIONS.md, the comments in `src/` and `bench/`,
   the change touches at most one document group (for example the design images, or README.md),
   one test group and one media rebuild (the images or the videos). You can do this work yourself
   in one turn.
5. **No doubt.** If one item is not true, or you are not sure, use the full track. The fast track
   is not a shortcut for a request that you do not understand.

Examples of the fast track: the text, the color or the position of one existing element; an
existing limit, time or default value; the order of the existing menu items; one message that the
app shows.

Examples of the full track: a new screen, panel, dialog, button, setting or input; a new state,
saved value or game rule; a change to the puzzles, the recognizer, the update, the installer or
the ink patch; a change that needs a new image or more than one image group; a different sequence
in the demo video.

If the user asks for the fast track, examine the same items. If one item is not true, tell the
user which item, and ask what to do. Do not use the fast track for a request that fails the
checklist.

Tell the user which track you use before you change the code. Write the answer to each item above
in the notes of the issue.

## Fast track (simple change)

The fast track replaces steps 1 to 12 of the full track below. Do all 5 steps. Do not start an
agent.

**F1. Prepare.** Make a beads issue (`bd create --type=feature` or `task`, then `bd update <id>
--claim`). Write the request of the user in its description. Record the baseline of the working
tree:

```bash
BASE=$(git stash create); BASE=${BASE:-$(git rev-parse HEAD)}
git status --porcelain > SCRATCH/dev-baseline-status.txt
```

Write `BASE` in the notes of the issue (`bd update <id> --notes`). Read the related sections of
REQUIREMENTS.md and INVARIANTS.md, and the related images (`images/INDEX.md`). Write their IDs in
the notes. Record each file that you change or add.

**F2. Change the code and install.** Change only what the request needs (step 4 of the full
track). Do not change the documents, the tests or `images/` yet. Install the dev build with the
`remarkable-install-dev` skill and a version name that tells the change (step 5 of the full track). Tell the user what changed and how to
see it. Tell the user to close Sudoku and open it again from AppLoad.

**F3. Get the approval on the tablet.** Ask the user (AskUserQuestion) if the result is correct.
Give the options "Correct", "Change it" and "Stop and revert". If the user asks for a change, do
F2 again. Change one thing at a time when the user is not sure what is wrong. Record each value
that the user rejected and the reason in the notes of the issue. If the user selects "Stop and
revert", do step 9 of the full track (revert), close the issue with a reason, and stop.

**F4. Keep the code and finish the change.** Do this only after the user selects "Correct".

1. Examine the code of the prototype as if another agent wrote it. Change it if it has any of
   these:
   - debug output, commented-out code, a value in two places, or an unused value;
   - a name that does not tell the behavior, or a comment that is not true now;
   - a change that the user did not approve.
   If you change the behavior, do F2 again. Also examine the edge cases that the user did not try
   (the limits, the other orientations, the completed game). If an edge case needs a decision that
   changes what the user sees, ask the user (AskUserQuestion).
2. Update the documents and the scripts that the change touches, with the table in "Documents"
   below. REQUIREMENTS.md and DECISIONS.md always. INVARIANTS.md if a rule changes. README.md and
   `site/` if they give the behavior or the instructions. The design image scripts and
   `images/INDEX.md` if a screen, a panel, a dialog or the icon looks different. The manual
   scripts if a manual page tells about or shows the change. The video event scripts if the demo
   shows the change, or if an event misses its target now (change the first part of
   `video/demo-full.txt` also).
3. Build the media group that changed: `make media-images` or `make media-video`. Read a crop of
   the changed area of the new images (`magick IMAGE -crop WxH+X+Y +repage SCRATCH/crop.png`), or
   some frames of the video. They must show the approved result. If they do not, change
   the scripts and build again.
4. Add or change the tests and the reference states (`src/reference.rs`, `tests/render.rs`) if the
   behavior or the pixels changed. Write each test from REQUIREMENTS.md and INVARIANTS.md, not
   from the code, so that the test can fail. Each test gives the rule IDs that it examines.
5. Run `cargo build --release --bins` with no warnings, `cargo test`, and `make snapshot`. The
   snapshot tool must agree with `images/`.
6. If the pixels, the reference states, the puzzles, the recognizer or `video/demo-full.txt`
   change, run `scripts/bench.sh` and compare with the baseline. Each changed hash must have a
   cause in the change. Then save the bench baselines again (refer to "Performance" in AGENTS.md).
   If `src/present.rs` or the platform code changes, run `make test-linux`.
7. If the binary changed after the approval, install the final dev build with the
   `remarkable-install-dev` skill and the same version name, and ask the user to examine it one time. The documents and the
   tests do not change the binary. A manual page change does.

**F5. Close and report.** Run `cargo test` one more time. Close the issue (`bd close`). Give the
changed files, the test results and the commands that you suggest (for example a commit). Do not
commit or push unless the user asks.

If a fact in F4 contradicts the test in "Choose the track" (for example the change needs a new
rule or a third document group), stop and tell the user. Do the rest with the full track from
step 8.

## Full track: phase 1 (prototype)

1. **Make a beads issue.** `bd create --type=feature` (or `task`), then `bd update <id> --claim`.
   In the description, write the request of the user with their words.

2. **Record the baseline of the working tree.** The working tree can contain other changes that
   are not yours. You must be able to remove only your changes in step 9. Run:

   ```bash
   BASE=$(git stash create); BASE=${BASE:-$(git rev-parse HEAD)}
   git status --porcelain > SCRATCH/dev-baseline-status.txt
   ```

   `git stash create` makes a commit of the working tree and does not change it. `SCRATCH` is the
   scratchpad directory. Write `BASE` in the notes of the issue (`bd update <id> --notes`). Record
   each file that you change or add from now on.

3. **Read the related part of REQUIREMENTS.md and the images.** Find the section, the images in
   `images/` (`images/INDEX.md`) and the invariants that the change touches. Do not change them.
   Write their IDs in the notes of the issue.

4. **Change the code.** Change only what the request needs (for example `layout.rs`, `render.rs`
   and `app.rs` for the UI, `game.rs` for the rules, `generator.rs` and `solver.rs` for the puzzles,
   `install.sh` for the installer). Before you change `present.rs`, the qtfb client or the ink
   patch, read the `remarkable-debug` skill. A change of the generator changes the puzzles of old
   saves: read PUZ-5 first.

   Go directly to step 5: the user must see the change on the tablet as fast as possible. Do not
   run `cargo build`, `cargo test` or the snapshot tool before the install. The Docker build
   compiles the code and stops if there is an error. Do not change the documents, the tests or
   `images/` in this phase.

5. **Install the dev build.** Use the `remarkable-install-dev` skill. Always build with the
   development profile (`Dockerfile.dev`, Cargo profile `tablet-dev`):

   ```bash
   docker build --platform linux/arm64 -f Dockerfile.dev --build-arg SUDOKU_VERSION=dev/update-icons --output type=local,dest=dist .
   ```

   Replace `update-icons` with a version name that tells the change (the `remarkable-install-dev`
   skill gives the rule). Use the same name for all builds of the change.
   `make install-dev DEV_NAME=update-icons` does the build and the install in one step. Do not use `Dockerfile`,
   `Dockerfile.test` or `make dist` for this phase: they are slow, and `Dockerfile.test` and
   `make dist` run the tests.

6. **Ask the user to examine the change on the tablet.** Tell the user:

   - what changed, and the steps that show it (for example: "Open a new game. Write a number in a
     cell. The cell highlight is now light gray."). If the tablet app does not show the change (for
     example the installer), give the command that shows it (for example `./install.sh` with
     `REMARKABLE_HOST`, or `tests/install/run.sh SCENARIO`);
   - to close Sudoku and open it again from AppLoad (the open app continues with the old program).

   Then ask (AskUserQuestion) if the result is correct. Give the options "Correct", "Change it"
   and "Stop and revert". Do not continue before the user answers.

   If the user asks for a change, do steps 4 to 6 again. Change one thing at a time when the user
   is not sure what is wrong. Record each value that the user rejected and the reason in the notes
   of the issue. If the user selects "Stop and revert", do step 9 (revert), close the issue with a
   reason, and stop.

7. **Review the approved prototype.** Do this only after the user selects "Correct", while the
   prototype is still in the working tree. Start one `dev-prototype-reviewer` agent. Give it:

   - the request of the user and the behavior and the values that the user approved;
   - the diff of the prototype only: `git diff $BASE` for the files that you changed, and the
     files that you added (not the other changes of the working tree);
   - the REQUIREMENTS.md sections and the invariant IDs of step 3.

   The agent examines only this change, not the full code base. Its report gives the requirements
   and the rules that change, the new rules, the illegal states, the edge cases (marked
   **USER DECISION** when the user must decide), the tests and the documents that must change.

   `git diff $BASE` compares the working tree with the baseline of step 2, thus it shows only your
   changes. The review does not change files. Do not fix the prototype code for the findings:
   step 9 reverts it. Put the findings into the description (step 8) as behavior and rules, not as
   code. If a finding needs a decision that changes what the user sees (for example an edge case
   that the user did not try), ask the user (AskUserQuestion) before step 8. If the answer changes
   the behavior, go back to step 4.

   This is the only review. Phase 2 has no review.

## Full track: phase 2 (implementation)

8. **Write the description of the change.** Write it to `SCRATCH/dev-<id>/description.md` and copy
   it to the design field of the issue (`bd update <id> --design`). The description tells **what** the
   user sees and what the app does, not **how** the prototype does it. Do not put code, diffs, function names or the
   names of variables in it. It must contain:

   - the request of the user, with their words;
   - the behavior that the user approved, screen by screen: what is visible, where, and when, and
     what each input does;
   - the exact values that the user approved: positions and sizes in pixels (1620x2160 at
     229 DPI), colors and gray levels, fonts and sizes, text, times, limits, default settings;
   - the values that the user rejected, and why (so that the team does not use them again);
   - the REQUIREMENTS.md sections, the invariant IDs and the images of step 3 that the change
     touches;
   - the files that the prototype changed (only the names, as a hint for the team);
   - how to see the change on the tablet (the steps of step 6);
   - the findings of the review (step 7), as behavior and rules;
   - the rows of "Documents" below that the change touches.

   Show the description to the user. The approval of the user on the tablet is the approval of the
   owner for these details (AGENTS.md: "ask the owner before you add it to REQUIREMENTS.md"). Write
   only the details that the user approved.

9. **Revert the prototype.** Remove only your changes. Do not touch the changes that were in the
   working tree before step 2.

   - For each file that you changed: `git checkout $BASE -- FILE`. This restores the file as it
     was in step 2, with the other changes of the user.
   - For each file that you added: delete it.
   - Compare `git status --porcelain` with `SCRATCH/dev-baseline-status.txt`. They must be equal.
     If they are not, stop and tell the user. Do not use `git checkout .`, `git reset --hard` or
     `git stash` (with no `create`).

10. **Give the work to a team of agents.** Use the Agent tool with the `dev-*` agent types of
   `.claude/agents/` (the table in "Agents" below), not `fork`: the agents must work from the
   description, not from the prototype. The definition of each agent type gives its task and its
   rules (STE, only its own files, no commit, no push, no change to the other changes in the
   working tree, report the problems in other files). In the prompt, give the absolute path of the
   handoff folder, the ID of its beads child issue and the files that it owns. Do not copy the
   description into the prompt. Make one beads child issue for each agent (`bd create
   --parent=<id>`).

   The handoff folder is `SCRATCH/dev-<id>/`. It lets the agents share context with no long
   searches. Each agent reads `description.md` and the notes of the waves before its own wave, and
   writes its own note (at most 40 lines), one file for each agent, thus parallel agents do not
   write the same file. The agents of wave 1 do not read each other's notes.

   Keep the context of this session small: each turn reads all of it again. The reports of the
   agents have at most 20 lines. Do not read a note, a full diff or a full image here unless a
   report shows a problem. Use `git diff --stat`.

   The team works in three waves. In a wave, the agents run in parallel (send all the Agent calls
   in one message). Start a wave only after all the agents of the wave before it are done.

   **Wave 1: the documents (3 agents, plus 1 agent for each other document group that the change
   touches, in parallel).** Each agent writes from the description, not from
   the work of the other agents. Before the wave, find the document groups that the change touches
   with the table in "Documents" below.

   1. **REQUIREMENTS.md** (`dev-requirements-writer`). The behavior, the screens, the sizes, the colors, the text, the times
      and the saved values that changed, in the related sections, also §15 (visual specification)
      and §17 (image reference) if the change is visible. It also owns DECISIONS.md: record what the user examined on the tablet and the
      values that the user rejected.
   2. **INVARIANTS.md** (`dev-invariants-writer`). The rules that change, and new rules for the new behavior, each with an
      ID and its source (R§…, "owner decision" with the date). Remove or change the rules that the
      old behavior makes true.
   3. **README.md** (`dev-readme-writer`). Update it only where the change makes it incorrect (for example "Features" or
      "Saved data"). It also owns `site/`, because README.md and `site/` must agree. If nothing
      changes, it reports "no change".
   4. **Design images** (`dev-image-scripts`, only if the change is visible on a screen, a panel or a dialog). It owns the
      scripts that make the design images (`scripts/ui-lib.sh`, `render.sh`, `toolbar.sh`,
      `board-state.py`, `analytics.py`, `scripts/icons/`, `icon.svg`) and `images/INDEX.md`. The
      images are the exact reference for the app, thus they change before the code.
   5. **Manual** (`dev-image-scripts`, only if a manual page tells about or shows the changed behavior). It owns
      `scripts/manual-*.sh` and `scripts/manual-lib.sh`. Change the portrait page and its
      landscape page together. The text of the manual is in STE. The app shows these images
      (`include_bytes!` in `src/render.rs`), thus the image build must be done before wave 2.

   The agents of wave 1 change only the scripts. They do not run the image build: two builds at
   the same time write the same files in `images/`. When the script agents are done, run
   `make media-images` one time, and send the paths of the changed images to the script agents
   (SendMessage). They read crops of the images and tell you if they show the approved result. If not, they
   change their scripts, and you run the build again.

   Then compare all the documents of wave 1 with each other and with the description (the
   `verify-claims` skill can do this). If they do not agree, send the difference to the agent that
   must fix it (SendMessage). REQUIREMENTS.md is correct when it and another document do not agree.

   **Wave 2: the implementation (1 agent, `dev-implementer`).** It changes the source in `src/` so
   that the code does what the updated REQUIREMENTS.md and INVARIANTS.md tell. It also changes
   `invariants.rs` for the changed rules, and it owns `install.sh` and the ink patch
   (`appload/sudoku/sudoku-penregion.qmd`) if the change touches them. `install.sh` must give the
   same instructions as the new README.md and `site/`. It does not change `tests/`, `images/`,
   `reference.rs` or the documents. The wave is done when `cargo build --release --bins` passes with no warnings
   and the dev build (step 5, with the version name of the change) passes. The tests can still fail now.

   **Wave 3: the tests and the media (N agents in parallel).** Select N from the
   change: one agent for each independent area, and each file has only one owner. Usually:

   - **Tests** (`dev-test-writer`), one agent for each group of test files that the change touches (for example
     `tests/behavior.rs` and `tests/common/mod.rs`; `tests/inv_*.rs` and `tests/properties.rs`).
     Change the tests that examine the old behavior and add tests for the new rules. Each test gives the
     rule IDs that it examines.
   - **Snapshots** (`dev-test-writer`). It owns `src/reference.rs` and `tests/render.rs`. Add or change a reference
     state if the current states do not show the change. `make snapshot` must agree with the new
     `images/` of wave 1. If the app does not agree with an image, send the difference to the
     implementation agent. Do not change the images or their scripts.
   - **Videos** (`dev-media`), only if the demo shows the change. It owns `video/`. Change `video/demo.txt` if
     its events do not show the change correctly now (for example a button moved), and change
     `video/demo-full.txt` also. Run `make media-video`.
   - **Installer** (`dev-test-writer`), only if `install.sh` changed. It owns `tests/install/`
     (also the fake `ssh` for a new or changed remote command) and `scripts/terminal.py`.
     `tests/install/run.sh` must pass for all scenarios. If the output of the installer changed,
     a `dev-media` agent runs `make media-installer` after it.
   - **Agent documents** (`dev-agent-docs`), only if the change makes them incorrect. It owns AGENTS.md (for example
     "Architecture Overview", the commands, the conventions) and the skills in `.claude/skills/`
     other than this skill (for example the lessons of `remarkable-debug` for a change of the
     screen updates, or `techniques/app-settings.md` for a change of the test settings). It also
     owns `.claude/agents/`.

   If a test agent finds a bug in `src/`, send it to the implementation agent (SendMessage). Do not
   let a test agent change `src/`.

   When wave 3 is done, start one more agent (`dev-bench`) that owns `bench/`. A change of
   the pixels, the reference states, the puzzles, the recognizer or `video/demo-full.txt` changes
   the output hashes, thus this agent must start
   after all the other agents. It saves the bench baselines again (refer to "Performance" in
   AGENTS.md), and runs `make test-linux` if `present.rs` or the platform code changed.

   Tell the user which wave runs now and which agents are in it. Do not do the work of an agent
   yourself. If an agent fails or stops, tell the user and ask what to do.

11. **Examine the result on the tablet.** Install the final dev build (step 5, with the version name of the change). Ask the user
    (AskUserQuestion) if the result is the same as the approved prototype. If it is not, send the
    difference to the implementation agent (SendMessage) and do this step again.

12. **Close and report.** Run `cargo test` and `make snapshot` one more time. Close the child
    issues and the issue (`bd close`). Give the changed files, the test results and the commands
    that you suggest (for example a commit). Do not commit or push unless the user asks.

## Agents

The agent types are in `.claude/agents/`. Each file gives the task, the input, the rules, the
report and, for some agents, the model and the effort of its agent. If the procedure of a phase
changes, change the agent file also. `dev-test-writer` always uses Opus (owner decision).

The fast track does not start an agent. The agents below belong to the full track.

| Agent type                | Step or wave      | Owns                                                         |
| ------------------------- | ----------------- | ------------------------------------------------------------ |
| `dev-prototype-reviewer`  | Step 7 (phase 1)  | Nothing (read-only review of the prototype diff)             |
| `dev-requirements-writer` | Wave 1            | REQUIREMENTS.md, DECISIONS.md                                |
| `dev-invariants-writer`   | Wave 1            | INVARIANTS.md                                                |
| `dev-readme-writer`       | Wave 1            | README.md, `site/`                                           |
| `dev-image-scripts`       | Wave 1            | The design image scripts, or the manual scripts (one group)  |
| `dev-implementer`         | Wave 2            | `src/` (not `reference.rs`), `install.sh`, the ink patch     |
| `dev-test-writer`         | Wave 3            | One group of tests (also `reference.rs`, `tests/install/`)   |
| `dev-media`               | Wave 3            | `video/`, `images/install-*.png`                             |
| `dev-agent-docs`          | Wave 3            | AGENTS.md, `.claude/skills/` (not this skill), `.claude/agents/` |
| `dev-bench`               | After wave 3      | `bench/`                                                     |

Do not use `pedantic-engineer` in this skill: it audits the full code base. Use it only when the
user asks for an audit.

## Documents

Examine each row for each change. In the description (step 8), give the rows that the change
touches. If a document has no owner in the waves of step 10, the change does not touch it. Make
sure of this before you start the team.

On the fast track, you own each row, and you do its work in F4. The rows that need work beyond
REQUIREMENTS.md, DECISIONS.md, the comments in `src/` and `bench/` must be at most one. If they
are more, stop and use the full track.

| Document                                                  | It changes when                                          | Owner (wave)        |
| --------------------------------------------------------- | -------------------------------------------------------- | ------------------- |
| REQUIREMENTS.md (also §15 and §17)                        | Always                                                   | REQUIREMENTS (1)    |
| DECISIONS.md                                              | Always (what the user examined, the rejected values)     | REQUIREMENTS (1)    |
| INVARIANTS.md                                             | A rule changes or a new rule is necessary                | INVARIANTS (1)      |
| README.md, `site/index.html`                              | A feature, a setting or a saved value that they give     | README (1)          |
| Design image scripts, `icon.svg`, `images/INDEX.md`       | A screen, a panel, a dialog or the icon looks different  | Design images (1)   |
| `scripts/manual-*.sh` (the in-app manual)                 | A manual page tells about or shows the changed behavior  | Manual (1)          |
| Comments in `src/`                                        | The code that they tell about changes                    | Implementation (2)  |
| `install.sh`, the ink patch                               | The installation or the ink patch changes                | Implementation (2)  |
| `tests/install/`, `scripts/terminal.py`, `install-*.png`  | `install.sh` changes                                     | Installer (3)       |
| `video/demo.txt`, `video/demo-full.txt`                   | The demo shows the change, or its events miss a button   | Videos (3)          |
| AGENTS.md, `.claude/skills/`, `.claude/agents/`           | The architecture, a command or a debug lesson changes    | Agent documents (3) |
| `bench/` (the baselines)                                  | The pixels, states, puzzles or recognizer change         | Bench (after 3)     |

Do not change this skill in the team: if the procedure must change,
tell the user.

## Rules

- Never update REQUIREMENTS.md, INVARIANTS.md, the tests or `images/` before the user approves
  the change on the tablet.
- On the full track, never give the code or the diff of the prototype to the team. The description
  and the handoff notes of the earlier waves are the only input.
- Never remove changes in the working tree that were there before step 2 (or F1).
- If you cannot connect to the tablet, tell the user. Then run
  `cargo run --release --bin snapshot -- DIR` with a `DIR` in the scratchpad, give the full paths
  of the PNG files that show the change, and ask if the images are sufficient for the approval. Do
  not decide this yourself, and do not copy the files into `images/`.
- If the user approves only a part, keep only that part: the description contains only that part
  on the full track, and the code contains only that part on the fast track.
