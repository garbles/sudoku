---
name: verify-claims
description: Narrow checker that takes a short list of specific claims (for example "`make bench` runs the bench", "the app has an eraser undo", "INVARIANTS.md MRK-7 agrees with REQUIREMENTS.md §5") and finds each place in the documentation, the scripts and the code where a claim is not consistent. It examines only the given claims and nothing more. Use when the user or an agent gives one or more claims to verify. Argument - the claims. It does not change files.
---

# Verify claims

Write all messages in ASD-STE100 Simplified Technical English, with US spelling (refer to
AGENTS.md).

## Your task

You get a short list of claims in the arguments of this skill. If there are no arguments, use the
claims in the last message of the user. If there are no claims, ask the user for them and stop. For each claim, find all the places where the repository does not
agree with the claim, or where two places do not agree with each other about the claim.

Examine **only the given claims**. This is your most important rule.

- Do not report a problem that is not about a given claim, also if it is a real problem. At the
  end, you can give a maximum of three such problems in one line each, under "Not examined".
- Do not review style, wording, tests or code quality.
- Do not expand a claim. If the claim is "`make test` runs `cargo test`", do not examine the other
  targets of the Makefile.
- If a claim is not clear, write your interpretation in one sentence, then examine that
  interpretation. Do not stop to ask.

Do not change files. Do not commit. Report only.

## Types of claims and where to look

1. **Commands** (for example "`cargo run --release --bin snapshot -- DIR` draws the reference
   states").
   - Find each place that gives the command: `README.md`, `AGENTS.md`, `CLAUDE.md`, `Makefile`,
     `site/index.html`, `install.sh`, `scripts/`, `.github/workflows/`, `Dockerfile*`,
     `.claude/agents/`, `.claude/skills/`, comments in `src/`.
   - Make sure that the command can run: the binary is in `Cargo.toml` or `src/bin/`, the script
     exists, the Makefile target exists, the flags and the arguments are in the argument parser,
     the files that the command uses exist.
   - Run the command only if it is fast, read-only and local (for example `--help`, `make -n`,
     `cargo run --bin X -- --help`, `bash -n script.sh`). Do not run Docker builds, installs on the
     tablet, network commands, `--save` or commands that write to tracked files. If you cannot run
     it, tell that you examined it by reading only.
   - Compare all the places that give the command: same flags, same paths, same environment
     variables.
2. **Features of the app** (for example "the player can hide pencil marks").
   - Find the feature in `REQUIREMENTS.md`, `INVARIANTS.md`, `README.md`, `site/index.html`, the
     manual text (`scripts/manual-*.sh`) and `images/INDEX.md`.
   - Find the code that does it in `src/`. Read the code. Do not trust a name or a comment.
   - Report each place that tells something different: a different value, a different name, a
     missing step, a feature that one place gives and another place does not have.
3. **Invariants and requirements** (for example "MRK-7 agrees with REQUIREMENTS.md").
   - Read the rule in `INVARIANTS.md` and each related section of `REQUIREMENTS.md`.
   - When they do not agree, REQUIREMENTS.md is correct (AGENTS.md). Report the change for
     INVARIANTS.md.
   - Look at the check in `src/invariants.rs` and the tests that give the rule ID
     (`grep -rn "RULE-ID" src tests`) only to find out if they agree with the claim.

Also apply the agreement rule of AGENTS.md when a claim is about the user instructions: README.md,
`install.sh` and `site/` must give the same instructions.

## How to work

1. Write the list of claims, with a number for each one (`C1`, `C2`, ...).
2. For each claim, search with `grep -rn` for the key words, the command name, the flags, the rule
   ID and the file names. Use a small number of good searches. Read each match in its context.
3. Before you report an inconsistency, prove it with the quoted text and `file_path:line` of each
   side.
4. Stop when you examined each claim. Do not continue to other subjects.

## Report format

For each claim:

### Cn: <the claim, as given>

- **Verdict**: Consistent, Inconsistent, False (the claim itself is wrong) or Cannot verify.
- **Interpretation** (only if the claim was not clear): one sentence.
- **Places examined**: a short list of `file_path:line`.
- **Inconsistencies**: for each one:
  - `file_path:line`: quoted text.
  - `file_path:line`: quoted text that does not agree (or the code, or the command output).
  - **Problem**: one sentence.
  - **Correction**: which place to change, and to what. If the correct value is not clear, write
    "Ask the owner".
- **How verified**: read only, or the commands that you ran.

End with a table: claim | verdict | quantity of inconsistencies. Then, if necessary, "Not
examined" with a maximum of three lines.
