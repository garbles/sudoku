---
name: audit-doc-claims
description: Read README.md, INVARIANTS.md and REQUIREMENTS.md, put the claims that are the same, closely related or that seem to contradict each other into single items, and make a beads epic with one task for each item. In goal mode, it also works through all the tasks with subagents (a maximum of 5 at a time, a maximum of 3 tasks for each) that use the verify-claims and fix-inconsistency skills. Use when the user asks to audit, cross-check or reconcile the three documents. Argument - optional "goal" to work the tasks, or the ID of an existing epic to continue.
---

# Audit the claims of the documents

Write all messages, bead titles and bead descriptions in ASD-STE100 Simplified Technical English,
with US spelling (refer to AGENTS.md).

Use `bd` for all task tracking. Do not use TodoWrite, TaskCreate or Markdown lists for the tasks.
Do not commit, push or run `bd dolt push` unless the user tells you to.

## Mode

- **Plan mode** (the default): do steps 1 to 4. Then stop and report.
- **Goal mode**: the session has an active goal (`/goal`), or the arguments contain `goal`. Do steps
  1 to 5. The goal is complete only when all tasks of the epic are closed.
- If the arguments give the ID of an epic, do not make a new epic. Skip steps 1 to 3 and continue
  with the open tasks of that epic (`bd list --parent <epic-id> --status open,in_progress`).

## Step 1: Read the three documents

Read `README.md`, `INVARIANTS.md` and `REQUIREMENTS.md` completely. Do not sample. Do not read the
code or other documents in this step.

## Step 2: Make the items

A **claim** is one statement that a person can make true or false: a value, a limit, a default, a
behavior, a transition, a command, a file, a path, a format, a rule ID and its source section.

Put claims into **items**. One item contains all the claims that:

- tell the same thing in different places (for example README "7 difficulty modes", DIF-1 "seven
  modes", R§11 "seven modes"), or
- are about the same subject and must agree (for example R§8.3 "the board locks", CMP-3, INP-9,
  UND-8), or
- seem to contradict each other (for example two different values, defaults, names or orders).

Rules for the items:

- Each item has one subject. Make the subject narrow enough that one agent can verify it in one
  pass, and wide enough that the related claims of the three documents are together.
- Each claim with a contradiction must be in an item. A claim that is in only one document and has
  no related claim does not need an item.
- An invariant and the requirement section that it gives as its source are in the same item, if
  they tell the same thing.
- Do not put a claim into two items. If a claim is about two subjects, put it in the item of its
  main subject.
- Mark an item **suspect** when the claims seem to contradict each other. Give the reason in one
  sentence.

For each item, write:

- **Title**: `Verify: <subject>` (a maximum of 80 characters).
- **Claims**: a numbered list. Each claim has the place (`README.md:<line>`, `INVARIANTS.md
  <rule ID>` or `REQUIREMENTS.md §<section>`), and the claim in one sentence, near the words of the
  document.
- **Suspect**: yes or no, with the reason.

Before you continue, examine the list one time: merge items with the same subject, and split an item
that has more than approximately 8 claims.

## Step 3: Make the epic

```bash
bd create --type=epic --priority=2 \
  --title="Audit: consistency of README.md, INVARIANTS.md and REQUIREMENTS.md" \
  --description="Each child task is one group of claims that are the same, closely related or that seem to contradict each other. Each task: run the verify-claims skill on its claims, then the fix-inconsistency skill on each inconsistency that it finds."
```

Keep the epic ID.

## Step 4: Make one task for each item

Make the tasks with `--parent=<epic-id>`. Give priority 1 to a suspect item and priority 2 to the
other items. Put the claims in the description, so that an agent can work on the task with no other
context:

```bash
bd create --type=task --parent=<epic-id> --priority=<1|2> --labels=doc-audit \
  --title="Verify: <subject>" \
  --description="$(cat <<'EOF'
Claims:
1. <place>: <claim>
2. <place>: <claim>

Suspect: <yes|no>: <reason>

Procedure: run the verify-claims skill with these claims. For each inconsistency in its report, run the fix-inconsistency skill. Close this task when all claims are consistent, or when the only open items are questions for the owner (then make one bead with the label human for each question).
EOF
)"
```

In plan mode, report:

- the epic ID,
- the quantity of tasks, and the quantity of suspect tasks,
- a table: task ID | title | suspect.

Then stop.

## Step 5: Work the tasks (goal mode only)

Repeat until `bd list --parent <epic-id> --status open,in_progress` is empty.

1. Get the open tasks of the epic. Put them into batches of a maximum of **3 tasks**. Put tasks that
   are about the same section of a document into the same batch, so that two agents do not edit the
   same text at the same time.
2. Start a maximum of **5 subagents** at the same time (Agent tool, `general-purpose`, in the
   background). Give one batch to each subagent. When a subagent completes, start a subagent for the
   next batch. Never have more than 5 subagents in progress.
3. Give each subagent this prompt (replace the IDs):

   ```text
   Write all messages in ASD-STE100 Simplified Technical English (refer to AGENTS.md).
   You work on these beads tasks: <id1> <id2> <id3>. Work on each task in sequence:

   1. Run `bd update <id> --claim`. Run `bd show <id>` to get the claims.
   2. Use the Skill tool to run the skill `verify-claims` with the claims of the task as the
      argument. Examine only these claims.
   3. For each inconsistency in the report, use the Skill tool to run the skill `fix-inconsistency`,
      with that one inconsistency as the argument. Do one fix at a time.
   4. If a fix needs a decision of the owner, do not edit. Make one bead for the question, outside
      the epic: `bd create --type=task --priority=2 --labels=human,doc-audit
      --deps discovered-from:<id> --title="Owner decision: <subject>" --description="<question,
      the places that do not agree, the options>"`. Add its ID to the notes of the task.
   5. Record the result with `bd update <id> --append-notes "<verdicts, changed files, tests>"`.
      Then run `bd close <id> --reason "<one sentence>"`. Close the task also when the only open
      items are questions for the owner.

   Rules:
   - Other agents edit the same repository at the same time. Do not undo, reformat or revert changes
     that you did not make. Use Edit with exact text, not a rewrite of a full file.
   - Do not commit or push. Do not run Docker builds or install on the tablet.
   - Do not work on other tasks or other problems.

   In your last message, give for each task: the ID, the verdict of each claim, the changed files,
   the tests that you ran, and the questions for the owner.
   ```

4. When a subagent reports, read its report. Make sure that each of its tasks is closed
   (`bd show <id>`). If a task is not closed and the subagent did not give a reason, put the task
   into a new batch one more time. If it fails two times, record the reason in its notes, and leave
   it open for the user.
5. When all tasks are closed, run `cargo test` one time on the full working tree. If a test fails,
   find the task that made the change, open it again (`bd update <id> --status open`) with a note,
   and continue at step 1.

When all tasks are closed and `cargo test` passes, close the epic (`bd close <epic-id>`) and report:

- the epic ID and the quantity of tasks,
- a table: task ID | verdict | changed files,
- all questions for the owner (`bd human list`),
- `git status --short`, and the commands for the user to make images, videos or bench baselines
  again, if a subagent reported that they are necessary.
