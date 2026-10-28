# Draws terminal output with ANSI colors as a PNG image of a terminal window:
#
#   python3 -I terminal.py IN.ansi OUT.png
#
# scripts/build.sh uses it for the images of the installer (images/install-*.png). The input comes
# from tests/install/run.sh. The script knows these SGR codes: 0, 1, 2, 22, 30-37, 39, 40-47, 49,
# 90-97, 100-107, 38;5;n and 48;5;n. It ignores all other escape sequences.
# The font is DejaVu Sans Mono (fonts/dejavu-mono-*.ttf). Each character has its own cell of the grid.
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
FONT = os.path.join(HERE, "fonts", "dejavu-mono-400.ttf")
BOLD = os.path.join(HERE, "fonts", "dejavu-mono-700.ttf")

COLS = 90            # the width of the terminal in characters
SIZE = 26            # the font size in pixels
ADVANCE = SIZE * 1233 / 2048   # the width of one character of DejaVu Sans Mono
LINE = 34            # the height of one line
PAD = 30             # the space between the window edge and the text
BAR = 52             # the height of the title bar
MARGIN = 40          # the space around the window (for the shadow)

BG = "#1a1b26"       # the theme: Tokyo Night
FG = "#c0caf5"
BAR_BG = "#24283b"
ANSI = ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
        "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5"]


def xterm(n):
    """The color of the xterm 256-color palette."""
    if n < 16:
        return ANSI[n]
    if n < 232:
        n -= 16
        levels = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (levels[n // 36], levels[n // 6 % 6], levels[n % 6])
    g = 8 + 10 * (n - 232)
    return "#%02x%02x%02x" % (g, g, g)


def mix(a, b, t):
    """The color between a (t = 0) and b (t = 1)."""
    pa = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(round(x + (y - x) * t) for x, y in zip(pa, pb))


def parse(text):
    """The rows of cells (char, fg, bg, bold, dim) of the text."""
    rows, row = [], []
    fg, bg, bold, dim = None, None, False, False
    for token in re.split(r"(\x1b\[[0-9;?]*[A-Za-z])", text):
        if token.startswith("\x1b["):
            if not token.endswith("m"):
                continue
            codes = [int(c) if c else 0 for c in token[2:-1].split(";")]
            i = 0
            while i < len(codes):
                c = codes[i]
                if c == 0:
                    fg, bg, bold, dim = None, None, False, False
                elif c == 1:
                    bold = True
                elif c == 2:
                    dim = True
                elif c == 22:
                    bold = dim = False
                elif 30 <= c <= 37:
                    fg = ANSI[c - 30]
                elif 90 <= c <= 97:
                    fg = ANSI[c - 82]
                elif c == 39:
                    fg = None
                elif 40 <= c <= 47:
                    bg = ANSI[c - 40]
                elif 100 <= c <= 107:
                    bg = ANSI[c - 92]
                elif c == 49:
                    bg = None
                elif c in (38, 48) and i + 2 < len(codes) and codes[i + 1] == 5:
                    if c == 38:
                        fg = xterm(codes[i + 2])
                    else:
                        bg = xterm(codes[i + 2])
                    i += 2
                i += 1
            continue
        for ch in token:
            if ch == "\n":
                rows.append(row)
                row = []
            elif ch == "\t":
                row.extend([(" ", fg, bg, bold, dim)] * (8 - len(row) % 8))
            elif ch != "\r":
                row.append((ch, fg, bg, bold, dim))
    if row:
        rows.append(row)
    while rows and not rows[-1]:
        rows.pop()
    return rows


# Box-drawing characters: the directions of their lines from the center of the cell, and the width
# of the lines. The script draws them as lines, so that they touch the next cell (as in a terminal).
BOX = {"─": "lr", "│": "ud", "╭": "dr", "╮": "dl", "╰": "ur", "╯": "ul",
       "━": "LR", "┃": "UD", "┏": "DR", "┓": "DL", "┗": "UR", "┛": "UL", "┯": "LRd", "┷": "LRu"}


def box_lines(ch, x, top, color):
    cx, cy = x + ADVANCE / 2, top + LINE / 2
    ends = {"u": (cx, top), "d": (cx, top + LINE), "l": (x, cy), "r": (x + ADVANCE, cy)}
    out = []
    for d in BOX[ch]:
        width = 3.2 if d.isupper() else 1.6
        ex, ey = ends[d.lower()]
        out.append(f"stroke {color} stroke-width {width} line {cx:.2f},{cy:.2f} {ex:.2f},{ey:.2f} stroke none")
    return out


def quote(ch):
    return {"\\": "\\\\", '"': '\\"', "%": "%%"}.get(ch, ch)


def main():
    rows = parse(open(sys.argv[1], encoding="utf-8").read())
    w = round(MARGIN * 2 + PAD * 2 + COLS * ADVANCE)
    h = MARGIN * 2 + BAR + PAD * 2 + len(rows) * LINE
    x0, y0 = MARGIN + PAD, MARGIN + BAR + PAD
    draw = [
        # The window, its title bar and the three buttons.
        f"fill {BG} roundrectangle {MARGIN},{MARGIN} {w - MARGIN},{h - MARGIN} 14,14",
        f"fill {BAR_BG} roundrectangle {MARGIN},{MARGIN} {w - MARGIN},{MARGIN + BAR} 14,14",
        f"fill {BAR_BG} rectangle {MARGIN},{MARGIN + BAR - 14} {w - MARGIN},{MARGIN + BAR}",
        f"fill #2f3549 rectangle {MARGIN},{MARGIN + BAR} {w - MARGIN},{MARGIN + BAR}",
    ]
    for i, color in enumerate(["#ff5f57", "#febc2e", "#28c840"]):
        cx, cy = MARGIN + 28 + i * 26, MARGIN + BAR // 2
        draw.append(f"fill {color} circle {cx},{cy} {cx + 8},{cy}")
    draw.append(f"fill #737aa2 font '{FONT}' font-size 20 text-anchor middle "
                f"text {w // 2},{MARGIN + BAR // 2 + 7} \"bash — install.sh\"")
    draw.append(f"text-anchor start font-size {SIZE}")
    for r, row in enumerate(rows):
        top = y0 + r * LINE
        for c, (ch, fg, bg, bold, dim) in enumerate(row):
            x = x0 + c * ADVANCE
            if bg:
                draw.append(f"fill {bg} rectangle {x:.2f},{top} {x + ADVANCE:.2f},{top + LINE - 1}")
            if ch == " ":
                continue
            color = fg or FG
            if dim:
                color = mix(color, BG, 0.45)
            if ch in BOX:
                draw.extend(box_lines(ch, x, top, color))
                continue
            draw.append(f"fill {color} font '{BOLD if bold else FONT}' text {x:.2f},{top + 25} \"{quote(ch)}\"")
    with tempfile.NamedTemporaryFile("w", suffix=".mvg", delete=False, encoding="utf-8") as f:
        f.write("\n".join(draw))
        mvg = f.name
    try:
        subprocess.run([
            "magick", "-size", f"{w}x{h}", "xc:white",
            # A soft shadow below the window.
            "-fill", "#00000030", "-draw", f"roundrectangle {MARGIN},{MARGIN + 8} {w - MARGIN},{h - MARGIN + 8} 14,14",
            "-blur", "0x14",
            "-draw", f"@{mvg}",
            "-units", "PixelsPerInch", "-density", "229",
            "-define", "png:exclude-chunks=date,time", sys.argv[2],
        ], check=True)
    finally:
        os.unlink(mvg)


main()
