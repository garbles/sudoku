# The screen update log of xochitl

xochitl can write one log line for each screen update that it sends to the panel: the region, the
flags, and the part of the region for each screen mode (for example ` - ui ........:` or
` - pen (ui) ..:`). This log shows what really gets to the panel. It is not the same as what the app
sends (refer to the lessons in SKILL.md).

The log category is `rm.framebuffer.updates`. Its debug messages are off by default.

## Turn the log on

1. Write a systemd drop-in file in the service folder of xovi. The xovi start script copies this
   folder each time that it starts xochitl:

   ```bash
   ssh ... root@HOST 'printf "[Service]\nEnvironment=\"QT_LOGGING_RULES=rm.framebuffer.updates.debug=true\"\n" > /home/root/xovi/services/xochitl.service/90-sudoku-debug.conf'
   ```

2. Restart xochitl with xovi: `ssh ... root@HOST /home/root/xovi/start`. **This closes all the open
   documents and apps.** Ask the owner before you do it.

3. Make sure that the log is on: `ls /home/root/xovi/services/xochitl.service/` shows
   `90-sudoku-debug.conf`, and `journalctl -u xochitl --since "-2min" | grep -c rm.framebuffer`
   is not 0 after a change on the screen.

## Copy the log

Do the test on the tablet. Then copy the log to the computer (a file in the scratchpad):

```bash
ssh ... root@HOST 'journalctl -u xochitl --since "-10min" --no-pager -o short-precise' > xochitl.log
```

Use a relative time (`-10min`). The clock of the tablet can change while it sleeps or when it gets
the time from the network, thus an absolute time (`--since "20:49"`) can find no lines.

## Turn the log off

Delete `90-sudoku-debug.conf` and restart xochitl with `xovi/start` (ask the owner first).

Do not use `~/.config/QtProject/qtlogging.ini`. The xochitl service has no `HOME` variable, so Qt
does not read this file.
