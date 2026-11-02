# Runs a scenario of run.sh in a terminal (a pseudo-terminal of 90 columns) and pushes keys. Thus the
# installer shows its menus for the arrow keys. Writes the terminal output to stdout:
#
#   python3 -I tests/install/tty.py SCENARIO KEY...
#
# Keys: up, down, enter, q. Before each key, the script waits until the menu shows its hint line
# again (one time for each key). After the last key, it waits until the installer stops. With the
# last key "wait", it stops the installer when the menu shows its hint line (for an image of an open
# menu). scripts/build.sh makes the images install-menu.png and install-select.png with this script.
import os, pty, select, signal, struct, sys, time, fcntl, termios

HERE = os.path.dirname(os.path.abspath(__file__))
KEYS = {"up": b"\x1b[A", "down": b"\x1b[B", "enter": b"\r", "q": b"q"}
HINT = "Enter select".encode()
TIMEOUT = 30

scenario, keys = sys.argv[1], sys.argv[2:]
pid, fd = pty.fork()
if pid == 0:
    os.environ["RUN_TTY"] = "1"
    os.execvp("bash", ["bash", os.path.join(HERE, "run.sh"), scenario])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 90, 0, 0))
out = b""


def read(until):
    """Reads the output until the condition is true or the installer stops. Returns False when it stops."""
    global out
    end = time.time() + TIMEOUT
    while not until():
        if time.time() > end:
            sys.exit("tty.py: timeout")
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                return False
            if not data:
                return False
            out += data
    return True


wait = keys and keys[-1] == "wait"
if wait:
    keys = keys[:-1]
for n, key in enumerate(keys):
    read(lambda: out.count(HINT) > n)
    os.write(fd, KEYS[key])
if wait:
    read(lambda: out.count(HINT) > len(keys))
    os.kill(pid, signal.SIGKILL)
else:
    read(lambda: False)
os.waitpid(pid, 0)
sys.stdout.buffer.write(out)
