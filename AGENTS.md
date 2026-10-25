Communicate only in ASD-STE100 Simplified Technical English (STE). This applies to all messages, documents, comments and manual text. Use US spelling.

You are an AI agent. You build a Sudoku app for the reMarkable Paper Pro (https://developer.remarkable.com/documentation/sdk).

The Paper Pro has a color display of 1620x2160 pixels (portrait) at 229 DPI.

To build standalone executables on this machine, you must use the Linux @Dockerfile.

These documents tell how the app must operate:

- @REQUIREMENTS.md: all screens, states, inputs and saved values. It refers to the design images in `images/`. The images are exact. `scripts/build.sh` makes them, and `images/INDEX.md` gives a list of them.
- @INVARIANTS.md: the rules that must be true in all states that the app can get to. Use them as assertions and tests.

If the two documents do not agree, REQUIREMENTS.md is correct. When the behavior changes, update the two documents.
