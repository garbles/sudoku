# Measure the timing of a stroke from the journal

Use this to find where the time goes between the pen and the panel.

1. Turn on the input log (`log-input`, [app-settings.md](app-settings.md)) and the update log
   ([xochitl-update-log.md](xochitl-update-log.md)). Close the app and open it again.
2. Write on the tablet. Then read the journal with the time of each line:
   `journalctl -u xochitl -o short-precise --since "-2min"`.
3. Pair each pen event (pen down, pen up) with the next `swapBuffers` line by the journal time. The
   difference is the time from the pen to the panel call.
4. A touch from the hand shows as a finger event approximately 97 ms after a pen up.
5. Before you trust the numbers, make sure that the pen area is on
   ([pen-area.md](pen-area.md)) and that the running Sudoku is the new build.
