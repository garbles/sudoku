---
name: fix-inconsistency
description: Fixes exactly one inconsistency between the documentation, the scripts and the code (for example one finding from the verify-claims skill). It makes the smallest change that makes all the places agree, then verifies it. Use when the user or an agent gives one specific inconsistency to fix. Argument - the inconsistency. It does not fix other problems and it does not commit.
---

# Fix one inconsistency

Write all messages, comments and document text in ASD-STE100 Simplified Technical English, with US
spelling (refer to AGENTS.md).

## Your task

You get **one** inconsistency in the arguments of this skill. If there are no arguments, use the
inconsistency in the last message of the user. Usually it is one finding from the
`verify-claims` skill: two or more places (`file_path:line`) that do not agree, and a correction. Make all the
places that are about this inconsistency agree. Then verify the change.

Fix **only this one inconsistency**. This is your most important rule.

- If you get more than one inconsistency, fix only the first one. List the others in your report
  as "Not fixed".
- Do not fix other problems that you see, also if they are real. You can give a maximum of three in
  one line each, under "Seen, not fixed".
- Do not reformat, rename or reword text that is not part of the inconsistency.
- Do not commit, push or run `bd` commands that change issues.

## Which side is correct

Decide which side is correct before you edit. Use these rules in this order:

1. The input tells which side is correct, and the repository does not show that this is wrong.
2. REQUIREMENTS.md is correct when it does not agree with INVARIANTS.md (AGENTS.md).
3. For a command, the code that runs is correct: `Cargo.toml`, `src/bin/`, the argument parser,
   the `Makefile`, the scripts. Change the documents to agree with it, unless the input tells you
   to change the command.
4. For a feature, REQUIREMENTS.md is correct. If the code does not obey it, change the code.
5. If the correct value is not clear, or the fix needs a detail that the requirements do not give,
   **stop**. Do not edit. Report "Ask the owner" with the question and the options (AGENTS.md:
   ask the owner before you add a detail to REQUIREMENTS.md or INVARIANTS.md).

## Rules from AGENTS.md that apply to a fix

- README.md, `install.sh` and `site/index.html` must give the same instructions. If you change the
  instructions in one of them, change the other two in the same fix. If `install.sh` changes a
  remote command, also change the fake ssh in `tests/install/fake/`.
- When the behavior changes, update REQUIREMENTS.md and INVARIANTS.md.
- When a change makes README.md incorrect, update README.md.
- Do not edit generated files: `appload/sudoku/icon.png`, `images/pro/*.png`, `video/*`. If the fix
  needs new images, videos or bench baselines (a change of pixels, reference states, puzzles or
  `video/demo-full.txt`), do not make them. Report the command that the user must run (refer to
  the Makefile and AGENTS.md).
- `version.json` format: do not change it without approval from the owner. Stop and report.

## How to work

1. Write the inconsistency in one sentence. Write which side is correct, and why (refer to the
   rules above).
2. Read each place that the input gives. Then search with `grep -rn` for other places that tell the
   same thing (the same command, flag, value, rule ID or feature name). A fix that changes one place
   and leaves a third place wrong is not complete.
3. Make the smallest edits that make all the places agree.
4. Verify:
   - `grep -rn` again: no place tells the old, incorrect thing.
   - For a command: run it if it is fast, local and read-only (`--help`, `make -n`, `bash -n`).
     Do not run Docker builds, installs on the tablet or network commands.
   - If you changed a file in `src/`, `tests/`, `Cargo.toml` or `install.sh`: run `cargo test`
     (and `tests/install/run.sh` for the installer scenarios that apply). If a test fails, fix
     your change or undo it. Do not change a test only to make it pass.
   - If you changed only Markdown, HTML or comments, `cargo test` is not necessary.
5. Run `git diff --stat` and make sure that each changed file is part of this fix.

## Report format

- **Inconsistency**: one sentence.
- **Correct side**: which place, and the rule that decided it.
- **Changes**: for each file, `file_path:line` and a one-line description.
- **Verification**: the commands that you ran and the results. If you did not run a command, tell
  why.
- **Actions for the user** (if necessary): commands to run (for example to make images or bench
  baselines again), or the question for the owner.
- **Not fixed** / **Seen, not fixed** (if necessary): one line each.
