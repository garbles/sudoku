# Read the update log

The log of xochitl has many lines. Keep only the screen updates and the pen events, and remove the
prefixes:

```bash
grep -E 'swapBuffers|  - |type 0x2[01]|sudoku: ' xochitl.log \
  | sed 's/^[A-Z][a-z]* [0-9]* //; s/ [a-z0-9-]* xochitl\[[0-9]*\]://; s/rm.framebuffer.updates//; s/EPFramebuffer::PresentationMode::Default//' \
  | cut -c1-140
```

The pen events come only with the `log-input` setting ([app-settings.md](app-settings.md)). The
app and xochitl write to the same log: their lines can mix in one line.

## What to look for

Make a time line around the problem: the pen down (`0x20`) and pen up (`0x21`) of each stroke, and
each update between them. For each update, read:

- **The screen mode.** `pen (ui)` is the Pen waveform (fast, no gaps). `ui` is the UI waveform. Ink
  in `ui` while the pen moves can have gaps.
- **The size.** An ink update is small (some tens of px). A large update while the pen moves is a
  redraw. If it covers the cell where the player writes, it can stop the ink there.
- **The delay.** The first ink update must come approximately 30 to 80 ms after the pen down. If no
  update comes until the pen up, the ink waited (for example for an update in progress).
- **The pen area.** The start and the stop of the pen area show as one `pen (ui)` or `ui` update of
  the full area (the inside of a cell, for example 144x144). Ink before the start of the area goes
  with the UI waveform.

## Patterns that we found

| Pattern in the log | Cause |
|---|---|
| Two updates of one tick never show, only their bounding box | Qt merges the updates of one frame. |
| Ink in `ui`, then the 144x144 `pen (ui)` start of the area | The pen area started too late (after the first ink). |
| A wide `ui` update over the next cell, then no ink until the pen up | A redraw stopped the ink: the ink waited for it. |
| `pen (ui)` updates over gray or colored pixels | Ghosts: the Pen waveform cannot drive colors. |

## Compare with the app

The rectangles in `[QTFB]: Updated region x y w h` are what the app sent. The rectangles in
`swapBuffers` are what Qt sent to the panel, after the merge and with 1 px more at the right and the
bottom. A difference between the two shows a merge of Qt.
