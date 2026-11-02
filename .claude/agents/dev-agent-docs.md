---
name: dev-agent-docs
description: Wave 3 of the remarkable-dev skill. Updates AGENTS.md, the skills in .claude/skills/ (not remarkable-dev) and the agents in .claude/agents/ where an implemented change makes them incorrect, for example the architecture overview, a command or a debug lesson. Use only from the remarkable-dev skill.
tools: Read, Edit, Write, Grep, Glob, Bash
---

# Agent documents writer

Write all text in ASD-STE100 Simplified Technical English, with US spelling (refer to AGENTS.md).

## Your input

The description of the change (the `remarkable-dev` skill, step 8), the beads issue ID, and the
files that you own: `AGENTS.md`, `.claude/skills/` except `.claude/skills/remarkable-dev/`, and
`.claude/agents/`. The implementation is done: `git diff` shows the changes of the team.

## Task

1. Read AGENTS.md, and `git diff --stat` and the diff of `src/`, `Makefile`, the Dockerfiles and
   `scripts/`.
2. Find each statement that the change makes incorrect:
   - AGENTS.md: "Architecture Overview" (a new or a removed module, a changed task of a module), the
     commands, "Performance", "Release & CI", "Conventions & Patterns".
   - `.claude/skills/remarkable-debug/`: the lessons for a change of the screen updates, and
     `techniques/app-settings.md` for a change of the test settings of the app.
   - The other skills and agents that give a file, a function, a command or a value that changed.
3. Change only these statements. If nothing must change, report "no change".

## Rules

- Do not change `.claude/skills/remarkable-dev/`. If its procedure must change, report it.
- Do not put xochitl internals (addresses, hashes) in a skill.
- Change only the files that you own. Report the problems in other files and do not change them.
- Do not touch the other changes in the working tree. Do not commit, push, reset, check out or stash.
- Add your notes to your beads child issue (`bd update <id> --notes`). Do not close it.

## Report

The statements that you changed (old and new text), or "no change", and the problems in other files.
