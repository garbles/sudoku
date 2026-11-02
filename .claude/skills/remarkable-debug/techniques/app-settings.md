# Test settings of the app

The app reads these settings when it starts. Each one is an environment variable, or a file in the
data folder (`/home/root/.local/share/rm-sudoku/`). A file is easier on the tablet, because AppLoad
starts the app. Close the app and open it again after a change. Delete the file to go back to the
default.

| File | Variable | Contents | Default |
|---|---|---|---|
| `log-input` | `SUDOKU_LOG_INPUT` | (any) Writes each input message to the system log. | off |
| `refresh-mode` | `SUDOKU_REFRESH_MODE` | qtfb refresh mode: `ufast` (`pen`), `fast` (`mono`), `animate`, `content`, `ui`. | `ui` |
| `ink-wave-ms` | `SUDOKU_INK_WAVE_MS` | The time in ms that the app gives a screen update to complete (`present::INK_WAVE_MS`). | 400 |

Example:

```bash
ssh ... root@HOST 'echo 600 > /home/root/.local/share/rm-sudoku/ink-wave-ms'
ssh ... root@HOST 'touch /home/root/.local/share/rm-sudoku/log-input'
```

When it starts, the app writes the selected values to its standard error output. AppLoad runs the
app in xochitl, thus these lines are in the log of xochitl:

```
sudoku: pen region on
sudoku: refresh mode Ui
```

With `log-input`, each pen event is a line `sudoku: input type 0x.. id .. at (x, y) d ..`. The types
of the pen: `0x20` pen down, `0x22` pen move, `0x21` pen up. The types `0x10` to `0x12` are touch
events. The positions are physical pixels.
