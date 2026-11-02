---
name: improve-architecture
description: Find refactors that make the Rust code easier to read and safer with no change of behavior, and make a beads epic with one task for each refactor. In goal mode, it also does the tasks, one at a time, with subagents, and keeps a refactor only when all tests pass, the output hashes of the bench do not change and a review agrees that the code is easier to read. Its principles are YAGNI, "make illegal states unrepresentable" (enums with data, newtypes, Option in place of sentinel values), one source of truth, names that tell the intent and abstractions that remove duplicate code. Use when the user asks to improve, simplify, clean up or refactor the architecture or the code base, to make the code easier to read or understand, to remove dead code or duplicate code, or to make illegal states unrepresentable. Argument - optional "goal" to do the tasks, "commit" to commit each kept refactor, or the ID of an existing epic to continue.
---

# Improve the architecture

Write all messages, comments, bead titles and bead descriptions in ASD-STE100 Simplified Technical
English, with US spelling (refer to AGENTS.md).

Use `bd` for all task tracking. Do not use TodoWrite, TaskCreate or Markdown lists for the tasks.
Do not push or run `bd dolt push`.

## The goal

Make the code **easier to read**, and make incorrect states impossible to write, **with no change
of behavior**.

Code is easy to read when a person who does not know it can find the answer to a question quickly
and be sure of the answer. For example: "Which states can the update row have?", "What occurs when
the pen touches a filled cell?", "Where does the app save the settings?". A refactor improves
readability when, after the change:

- a reader must keep fewer items in mind to understand one function or one type,
- the types tell which values are possible, thus the reader does not have to find a comment or a
  check in a different file,
- each fact (a rule, a size, a list of buttons) is in one place only,
- the names tell the intent ("what" and "why"), not the mechanism,
- the structure of the code follows the structure of REQUIREMENTS.md, thus a reader can go from a
  requirement to its code,
- there is less nesting, and a function does one thing.

The quantity of lines is not the measure. A shorter version that is more difficult to read is
worse. A longer version is better if it makes the code clearer, for example an enum with one
variant for each state in place of two `bool` fields.

A refactor that only changes the format, or that renames an item with no better meaning, is not
an improvement.

## Principles

1. **YAGNI.** Remove what the app does not use now: dead functions, parameters that always get the
   same value, fields that no code reads, generic parameters and traits with only one
   implementation, configuration options with only one value, `pub` items that only one module
   uses, code "for later". Each item that stays is an item that a reader must understand. Do not
   add a hook, an option or a trait for a possible future need.
2. **Make illegal states unrepresentable.** When two or more fields together can have a value that
   is not correct, replace them with one type that has only the correct values:
   - An enum with data in place of a `bool` and an `Option` that must agree (for example
     `checking: bool` and `state: Unknown`).
   - `Option<T>` in place of a sentinel value (`0` for "empty", `-1`, `""`).
   - A newtype with a checked constructor in place of a raw integer with a range (a digit 1..9, a
     mode 0..6, a cell index 0..80). Check the value one time, at the edge (parsing, input), not
     in each function that uses it ("parse, don't validate").
   - Data that the code can calculate from other data is not kept in a second field (one source of
     truth). INVARIANTS.md CLS-1 is an example: the clashes come from `value`.
   - Use `match` with all variants, not `_ =>`, so that the compiler finds each place when a variant
     is added.
3. **One place for each fact.** When the same steps or the same values occur three or more times
   (the rule of three), put them in one function, one table of data or one loop. Two similar
   blocks do not always need an abstraction: a wrong abstraction is more difficult to read than a
   duplicate. Examples: a table of the toolbar buttons in place of one `if` for each button, one
   function for the portrait and the landscape layout with a parameter, one test helper in place
   of the same setup in each test.
4. **Names and structure that tell the intent.** A name tells what a value is for. A function has
   one level of abstraction: it calls steps with clear names, or it does the details, not the two.
   Use early returns in place of deep nesting. Put the items of one concept together, and keep the
   items of different concepts apart.
5. **Deep modules.** A module has a small interface and does much work. Remove a wrapper that only
   calls one other function. Merge two types that always change together.
6. **Use the standard library and the crates that the project has.** Do not write by hand what
   `std`, `Iterator` or a current dependency gives. Do not add a new dependency for a refactor.

## Constraints

A refactor must not change these. They are the contract of the app:

- **Behavior.** REQUIREMENTS.md and INVARIANTS.md do not change. A refactor that needs a change of
  a requirement is not a refactor: make a bead with the label `human` and do not do it.
- **Pixels, puzzles and the recognizer.** The output hashes of the bench (`scripts/bench.sh`) do
  not change, and the snapshot tool agrees with `images/`. A seed gives the same puzzle (PUZ-5).
- **Saved data.** The format of `game.json`, `settings.json`, `results.sqlite`, the stamp file and
  `version.json` does not change, and the app reads the files of earlier versions (generator
  version 1, `auto_remove_on`). Look at the `serde` attributes before you change a type that the
  app saves. When an enum replaces fields of a saved type, keep the JSON the same (for example with
  `#[serde(from, into)]`) or do not do it.
- **Performance.** `present.rs`, `render.rs`, `recognizer.rs`, `generator.rs` and `solver.rs` are hot
  paths. A change there must not make the bench slower by more than the noise (approximately 3%).
- **Tests.** Do not remove a test or an assertion, and do not make a test weaker. The quantity of
  `#[test]` functions (`grep -rh '#\[test\]' src tests | wc -l`) does not decrease. You can merge
  the setup of tests into a helper. A test that gives a rule ID keeps it.
- **Documents.** If a refactor makes the Architecture Overview of AGENTS.md or README.md incorrect
  (a module name, a file), update it in the same change.
- **Generated files.** Do not edit `images/`, `video/`, `appload/sudoku/icon.png` or the bench
  baselines.

## Mode

- **Plan mode** (the default): do steps 1 to 4. Then stop and report.
- **Goal mode**: the session has an active goal (`/goal`), or the arguments contain `goal`. Do steps
  1 to 5. The goal is complete only when all tasks of the epic are closed.
- **Commit**: only when the arguments contain `commit` or the user tells you. Then make a branch
  `arch/<epic-id>` if you are on `main`, and commit each kept refactor (one commit for each task).
- If the arguments give the ID of an epic, do not make a new epic. Skip steps 1 to 3 and continue
  with the open tasks of that epic (`bd list --parent <epic-id> --status open,in_progress`).

## Step 1: Record the start

Run `git status --short` and count the tests (`grep -rh '#\[test\]' src tests | wc -l`). If the
working tree has changes, tell the user, because goal mode keeps or removes refactors relative to
these changes.

## Step 2: Read the code and find the refactors

Read each file in `src/` completely, the largest files first. Do not sample: a duplicate or a
second source of truth is visible only when you saw the two places. Then read `tests/` quickly for
setup that repeats.

While you read, write down the places where you had to stop and think: a value whose possible
states you had to find in a different file, a name that did not tell its purpose, a function that
you had to read two times. These places are the best candidates.

For each file, also look for:

- State that can be incorrect: `bool` flags that must agree with an `Option` or an enum, sentinel
  values, `unwrap()` and `expect()` on a value that "cannot" be `None`, `unreachable!()`,
  `_ =>` arms, checks in `invariants.rs` that a type could make unnecessary.
- Code that no one uses: `cargo clippy --all-targets` warnings, `#[allow(dead_code)]`, `pub` items
  with no user outside the module (`grep -rn`), parameters that each caller gives with the same value.
- Repeated steps: the same block in portrait and landscape, in left-handed and right-handed
  layouts, in each panel, in each screen, in each test.
- Long `if`/`match` chains that a table of data could replace.
- Long functions that mix levels of abstraction, and deep nesting.

Write each refactor as one **candidate**:

- **Title**: `Refactor: <what changes>` (a maximum of 80 characters).
- **Principle**: YAGNI, illegal state, one place, intent, deep module or std.
- **Places**: `file_path:line` of each place that changes.
- **Problem for the reader**: what is difficult to understand now, in one or two sentences.
- **Change**: what the new type, function or table is, in two to five sentences. Give the
  signature or the enum when it helps.
- **Risk**: low, medium or high, with the reason: saved data, hot path, pixels, many files.

Rules for the candidates:

- One candidate is one idea that one agent can do and verify in one pass. Split a large idea.
- Do not make a candidate that breaks a constraint. If the only way is to change behavior, make it a
  question for the owner (a bead with the label `human`), not a candidate.
- Do not make a candidate for a format change, or for a rename or a move with no better meaning.
- If two candidates change the same lines, merge them or make one depend on the other.
- Remove candidates with a small gain for the reader and a medium or high risk.

## Step 3: Make the epic

```bash
bd create --type=epic --priority=2 \
  --title="Architecture: code that is easy to read, with no illegal states" \
  --description="Start: <test_functions> tests. Each child task is one refactor with no change of behavior. Goal mode does the tasks one at a time and keeps a refactor only when the checks pass and a review agrees that the code is easier to read."
```

Keep the epic ID.

## Step 4: Make one task for each candidate

Priority: 1 for a large gain for the reader and low risk, 2 for the others, 3 for high risk.
Add dependencies (`bd dep add <task> <depends-on>`) when one refactor must come first, for example a
newtype before the refactors that use it.

```bash
bd create --type=task --parent=<epic-id> --priority=<1|2|3> --labels=architecture \
  --title="Refactor: <what changes>" \
  --description="$(cat <<'EOF'
Principle: <principle>
Places:
- <file_path:line>
Problem for the reader: <one or two sentences>
Change: <two to five sentences>
Risk: <low|medium|high>: <reason>

Procedure: follow step 5 of the improve-architecture skill (.claude/skills/improve-architecture/SKILL.md). Keep the refactor only when all checks pass and the review agrees that the code is easier to read. Else undo it and close this task with the reason.
EOF
)"
```

In plan mode, report:

- the epic ID,
- a table: task ID | title | principle | risk,
- the questions for the owner, if there are some.

Then stop.

## Step 5: Do the tasks (goal mode only)

Do the tasks **one at a time**. Refactors change the same files, and `cargo test` of one agent
would see the half-done edits of another agent. Thus there is never more than one subagent that
edits.

Repeat until `bd list --parent <epic-id> --status open` has no task:

1. Select the ready task with the highest priority.
2. Record the start: `BASE=$(git stash create)` (if it is empty, use `BASE=HEAD`). `git stash
   create` keeps the state of the working tree and does not change it.
3. Start one subagent (Agent tool, `general-purpose`) with this prompt (replace the values) and wait
   for its report:

   ```text
   Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).
   Read .claude/skills/improve-architecture/SKILL.md: the sections "The goal", "Principles" and
   "Constraints". You do one refactor: the beads task <id>.

   1. Run `bd update <id> --claim` and `bd show <id>`.
   2. Read all places of the task, and search (`grep -rn`) for each other user of the items that
      change. Make the refactor. If you find a clearer form of the same idea while you work, use
      it. Do not do other refactors: write them in your report.
   3. Run the checks:
      - `cargo test` (all tests must pass),
      - `cargo clippy --all-targets` (no new warnings),
      - `make snapshot` (the images agree),
      - if the change touches src/present.rs, src/render.rs, src/recognizer.rs, src/generator.rs,
        src/solver.rs, src/layout.rs, src/font.rs or src/reference.rs: `scripts/bench.sh` (the
        output hashes must not change, and no phase is slower by more than approximately 3%),
      - `grep -rh '#\[test\]' src tests | wc -l` (not fewer than before).
      If a check fails, fix the refactor. Do not change a test only to make it pass.
   4. If a check still fails, undo all your changes: `git restore --source=<BASE> --worktree -- .`
      and delete the new files that you made.
   5. Record the result: `bd update <id> --append-notes "<done or undone, checks, changed files,
      how the code is easier to read>"`. Do not close the task: the review closes it.

   Rules: do not commit or push. Do not run Docker builds or install on the tablet. Do not edit
   images/, video/ or the bench baselines. If the refactor needs a change of behavior or of a saved
   format, undo it, make a bead `bd create --type=task --priority=2 --labels=human,architecture
   --deps discovered-from:<id> --title="Owner decision: <subject>" --description="<question>"`, and
   report it.

   In your last message, give: the task ID, done or undone, the changed files, the checks and
   their results, how the code is easier to read (before and after, with a short example), and
   the other refactors that you saw.
   ```

4. **Review for readability.** If the subagent kept the refactor, start a second subagent
   (`general-purpose`, it does not edit) that did not see the work:

   ```text
   Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).
   Read the section "The goal" of .claude/skills/improve-architecture/SKILL.md. Then read the
   diff `git diff <BASE>` and the full functions and types around each change. You do not know
   why the change was made. Answer:
   1. Is the new code easier to read than the old code for a person who does not know it? Give
      one or two questions that a reader can answer more quickly now, or that are now more
      difficult.
   2. Does the change keep the behavior? Name each place where you have a doubt.
   3. Verdict: KEEP or UNDO, with one sentence.
   Do not change files.
   ```

   The verdict decides. If the verdict is UNDO, or the review has a doubt about the behavior that
   the tests do not answer, undo the refactor (`git restore --source=$BASE --worktree -- .` and
   delete the new files).
5. Verify it yourself. Do not trust the reports only:
   - `git diff --stat $BASE`: only the files of this refactor changed (none if it was undone).
   - If you have a doubt about a check, run it again.
   - Record the verdict: `bd update <id> --append-notes "<review verdict and reason>"`, then
     `bd close <id> --reason "<kept or undone, one sentence>"`.

   If the subagent did not finish, undo the changes, add a note, and open the task again one time.
   If it fails two times, close it with the reason "not possible with no change of behavior".
6. If the mode is **commit** and the refactor was kept: commit only the changed files, with the
   message `refactor: <title> (<id>)` and the attribution lines of the session.
7. If the subagents reported other refactors that agree with the principles and the rules of
   step 2, make a task for each of them in the epic.
8. Each 5 tasks, give the user one line: the tasks kept, the tasks undone, the tasks that remain.

When all tasks are closed:

1. Run all checks one time on the full working tree: `cargo test`, `cargo clippy --all-targets`,
   `make snapshot`, `scripts/bench.sh`. If one fails, find the refactor that caused it (`git diff`,
   or `git bisect` on the commits), undo it, open its task again with a note, and continue the loop.
2. Close the epic (`bd close <epic-id>`) and report:
   - a table: task ID | title | kept or undone | how the code is easier to read (one line),
   - the questions for the owner (`bd human list`),
   - `git status --short`,
   - the commands for the user, if they are necessary (for example `scripts/bench.sh --save` when
     only the speed changed, or the Linux bench of AGENTS.md for a hot-path refactor).
