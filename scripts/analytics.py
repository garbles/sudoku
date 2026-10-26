# Makes the analytics screens in the folder argv[1]:
#   analytics.png        summary: completed puzzles and average time, for all puzzles and for each difficulty
#   analytics-games.png  table of completed games (with a difficulty filter). Each game has a delete button.
#   analytics-delete.png the same list after a tap on a bin: a confirmation dialog
#   analytics-delete-all.png the summary after a tap on "Delete all games": a confirmation dialog
# and the same three screens for landscape (analytics-landscape.png, analytics-games-landscape.png,
# analytics-delete-landscape.png): 1872x1404 units with the same scale, on 2160x1620. The positions
# are the same as in src/layout.rs.
# All screens use one list of sample games (the same data as the SQLite results table). Thus the
# summary and the table agree. The layout uses the 1404x1872 units of the manual, with a scale to 1620x2160.
import os, random, subprocess, sys, datetime

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1]
FONT = os.path.join(HERE, "fonts", "rmsans-400.ttf")
HEAD = os.path.join(HERE, "fonts", "rmsans-500.ttf")
SCALE = 1.153846
STOPS = 5
PER_PAGE = 12

# --- sample results: (finished_at, mode 0-4, seconds)
rng = random.Random(7)
BASE_MINUTES = [9, 14, 20, 31, 47]
games = []
t = datetime.datetime(2026, 10, 24, 19, 30)   # the newest game. The other games are before this date.
for _ in range(23):
    t -= datetime.timedelta(days=rng.randint(1, 3))
    t = t.replace(hour=rng.randint(17, 22), minute=rng.randint(0, 59))   # games in the evening
    mode = rng.choices(range(5), weights=[3, 4, 6, 4, 2])[0]
    games.append((t, mode, int(BASE_MINUTES[mode] * 60 * rng.uniform(0.7, 1.4))))
games.sort(key=lambda g: g[0], reverse=True)

def clock(seconds):
    h, rem = divmod(int(round(seconds)), 3600)
    m, s = divmod(rem, 60)
    return f"{h}:{m:02d}:{s:02d}" if h else f"{m}:{s:02d}"

def text(out, x, y, size, color, anchor, s, font=FONT):
    out.append(f"stroke none fill {color} font '{font}' font-size {size} text-anchor {anchor} text {x},{y} \"{s}\"")

def meter(out, xa, y, level, step=34, on="black", off="gray60", hollow="white"):
    """The difficulty meter (same style as the game's), without labels."""
    xk, xb = xa + level*step, xa + (STOPS-1)*step
    out.append(f"fill none stroke-linecap round stroke {off} stroke-width 2.5 line {xk},{y} {xb},{y}")
    out.append(f"stroke {on} stroke-width 4.5 line {xa},{y} {xk},{y}")
    for i in range(STOPS):
        x = xa + i*step
        out.append(f"fill {on if i < level else hollow} stroke {on} stroke-width 2.5 circle {x},{y} {x+5},{y}")
    out.append(f"fill {on} stroke {hollow} stroke-width 2.5 circle {xk},{y} {xk+11},{y}")

# The page geometry (src/layout.rs): portrait 1404x1872 units, landscape 1872x1404 units.
class Page:
    def __init__(self, landscape):
        self.landscape = landscape
        self.w, self.h = (1872, 1404) if landscape else (1404, 1872)
        self.size = "2160x1620" if landscape else "1620x2160"
        self.right = self.w - 72                      # 1332 in portrait
        self.box_w = (self.w - 172) / 2               # 616 in portrait
        self.count_x = self.right - 432               # 900 in portrait
        self.seg_w = (self.w - 144) / 6               # 210 in portrait
        self.cols = 2 if landscape else 1             # the games list: one column of 12, or two of 6
        self.col_w = (self.w - 144 - 72 * (self.cols - 1)) / self.cols
        self.offsets = (370, 650, 780) if landscape else (488, 1038, 1218)   # meter, end of time, bin
        self.dialog = (self.w / 2 - 450, self.h / 2 - 176, self.w / 2 + 450, self.h / 2 + 174)
    def col_x(self, col):
        return 72 + col * (self.col_w + 72)
    def suffix(self, name):
        return name.replace(".png", "-landscape.png") if self.landscape else name

def n(v):
    """A number for MVG: integers without a decimal point (as in the first version of this script)."""
    return int(v) if float(v).is_integer() else round(v, 3)

def chrome(out, P, title, page=None):
    """'< Back' (returns to the game), the title, and optional page arrows."""
    out.append("fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline 48,37 38,48 48,59")
    text(out, 66, 56, 22, "black", "start", "Back", HEAD)
    text(out, 72, 170, 84, "black", "start", title, HEAD)
    if page:
        k, total = page
        cx, ny = n(P.w / 2), n(P.h - 37)
        text(out, cx, ny, 28, "gray50", "middle", f"{k} / {total}")
        prev = "gray80" if k == 1 else "black"; nxt = "gray80" if k == total else "black"
        out.append(f"fill none stroke-width 3 stroke-linecap round stroke-linejoin round stroke {prev} polyline {n(cx-74)},{n(ny-20)} {n(cx-84)},{n(ny-10)} {n(cx-74)},{ny} "
                   f"stroke {nxt} polyline {n(cx+74)},{n(ny-20)} {n(cx+84)},{n(ny-10)} {n(cx+74)},{ny}")

def hline(out, y, x0=72, x1=1332, w=1.7):
    out.append(f"stroke none fill black rectangle {n(x0)},{n(y)} {n(x1)},{n(y+w)}")

def render(out, P, name):
    mvg = os.path.join(OUT, f".{name}.mvg")
    with open(mvg, "w") as f:
        f.write(f"push graphic-context\nscale {SCALE},{SCALE}\n" + "\n".join(out) + "\npop graphic-context\n")
    subprocess.run(["magick", "-size", P.size, "xc:white", "-draw", f"@{mvg}",
                    "-units", "PixelsPerInch", "-density", "229", os.path.join(OUT, P.suffix(name))], check=True)
    os.remove(mvg)

ROW = 97  # 112 device px, the height of a list row on the device

def bin_icon(out, bx, cy):
    """Delete: a bin (lid, handle, body with two lines)."""
    out.append(f"fill none stroke black stroke-width 2.6 stroke-linecap round stroke-linejoin round "
               f"line {bx-14},{cy-11} {bx+14},{cy-11} polyline {bx-5},{cy-11} {bx-5},{cy-15} {bx+5},{cy-15} {bx+5},{cy-11} "
               f"polyline {bx-10},{cy-11} {bx-8},{cy+15} {bx+8},{cy+15} {bx+10},{cy-11} "
               f"line {bx-3},{cy-5} {bx-3},{cy+9} line {bx+3},{cy-5} {bx+3},{cy+9}")

def screens(P):
    # --- summary
    out = []
    chrome(out, P, "Statistics")
    total = len(games); avg = sum(g[2] for g in games) / total
    for x0, label, value in [(72, "Puzzles completed", str(total)), (n(100 + P.box_w), "Average time", clock(avg))]:
        out.append(f"fill none stroke black stroke-width 1.7 rectangle {x0},250 {n(x0+P.box_w)},440")
        text(out, x0+30, 305, 30, "gray40", "start", label)
        text(out, x0+30, 405, 84, "black", "start", value, HEAD)

    text(out, 72, 550, 50, "black", "start", "By difficulty", HEAD)
    text(out, 72, 615, 30, "gray40", "start", "Difficulty")
    text(out, n(P.count_x), 615, 30, "gray40", "end", "Completed")
    text(out, n(P.right), 615, 30, "gray40", "end", "Average time")
    y = 640; hline(out, y, 72, P.right)
    for mode in range(STOPS):
        times = [g[2] for g in games if g[1] == mode]
        cy = y + ROW//2
        text(out, 72, cy+12, 34, "black", "start", ["Easy", "", "", "", "Hard"][mode])
        meter(out, 230, cy, mode)
        text(out, n(P.count_x), cy+12, 34, "black", "end", str(len(times)))
        text(out, n(P.right), cy+12, 34, "black", "end", clock(sum(times)/len(times)) if times else "–")
        y += ROW; hline(out, y, 72, P.right)

    y += 70
    all_right = P.right - DELETE_ALL_W - 28
    out.append(f"fill none stroke black stroke-width 1.7 rectangle 72,{y} {n(all_right)},{y+ROW}")
    text(out, 102, y+ROW//2+12, 34, "black", "start", "All completed games", HEAD)
    ax = n(all_right - 40)
    out.append(f"fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline {ax},{y+ROW//2-12} {n(ax+12)},{y+ROW//2} {ax},{y+ROW//2+12}")
    # "Delete all games" at the right, in the same row (black: there are results)
    x0 = n(P.right - DELETE_ALL_W)
    out.append(f"fill none stroke black stroke-width 1.7 rectangle {x0},{y} {n(P.right)},{y+ROW}")
    bin_icon(out, x0+50, y+ROW//2)
    text(out, x0+90, y+ROW//2+12, 34, "black", "start", "Delete all games", HEAD)
    render(list(out), P, "analytics.png")

    # --- the "Delete all games" confirmation on the summary
    dialog(out, P, "Delete all games?", f"All {len(games)} completed games are deleted.")
    render(out, P, "analytics-delete-all.png")

    # --- table of completed games, filter "All", newest game first
    out = []
    pages = (len(games) + PER_PAGE - 1) // PER_PAGE
    chrome(out, P, "Completed games", (1, pages))
    # filter: All, then one segment for each difficulty. The selected segment is black, as on the device.
    seg_w, sx, sy = P.seg_w, 72, 240
    for i in range(STOPS + 1):
        x = n(sx + i*seg_w); selected = i == 0
        out.append(f"fill {'black' if selected else 'white'} stroke black stroke-width 1.7 rectangle {x},{sy} {n(x+seg_w)},{sy+80}")
        if i == 0:
            text(out, n(x+seg_w/2), sy+52, 32, "white" if selected else "black", "middle", "All", HEAD)
        else:
            meter(out, n(x+(seg_w-136)/2), sy+40, i-1, step=34)
    md, td, bd = P.offsets
    rows = games[:PER_PAGE]
    per_col = PER_PAGE // P.cols
    for col in range(P.cols):
        cx0 = n(P.col_x(col))
        if col == 0 or len(rows) > col * per_col:
            text(out, cx0, 395, 30, "gray40", "start", "Date")
            text(out, n(cx0+md), 395, 30, "gray40", "start", "Difficulty")
            text(out, n(cx0+td), 395, 30, "gray40", "end", "Time")
            hline(out, 420, cx0, cx0 + P.col_w)
    for i, (when, mode, secs) in enumerate(rows):
        col, r = divmod(i, per_col)
        cx0 = n(P.col_x(col)); y = 420 + r*ROW
        cy = y + ROW//2
        text(out, cx0, cy+12, 34, "black", "start", when.strftime("%-d %b %Y, %H:%M"))
        meter(out, n(cx0+md), cy, mode)
        text(out, n(cx0+td), cy+12, 34, "black", "end", clock(secs))
        bin_icon(out, n(cx0+bd), cy)
        hline(out, y+ROW, cx0, cx0 + P.col_w)
    render(out, P, "analytics-games.png")

    # --- delete confirmation for the second game in the list
    when, mode, secs = games[1]
    dialog(out, P, "Delete this game?", f"{when.strftime('%-d %b %Y, %H:%M')}, completed in {clock(secs)}.")
    render(out, P, "analytics-delete.png")

DELETE_ALL_W = 400   # the width of "Delete all games" (src/layout.rs)

def dialog(out, P, title, line):
    """A confirmation dialog at the center of the page, on the pale screen."""
    out.append(f"stroke none fill rgba(255,255,255,0.6) rectangle 0,0 {P.w},{P.h}")
    dx0, dy0, dx1, dy1 = (n(v) for v in P.dialog)
    out.append(f"fill white stroke black stroke-width 1.7 rectangle {dx0},{dy0} {dx1},{dy1}")
    text(out, dx0+40, dy0+75, 44, "black", "start", title, HEAD)
    text(out, dx0+40, dy0+140, 34, "gray25", "start", line)
    text(out, dx0+40, dy0+190, 34, "gray25", "start", "You cannot undo this action.")
    hline(out, dy1-ROW, dx0, dx1)
    out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {dx1},{dy1}")
    out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {(dx0+dx1)//2+1.7},{dy1}")
    text(out, (dx0*3+dx1)//4, dy1-ROW//2+12, 34, "black", "middle", "Cancel", HEAD)
    text(out, (dx0+dx1*3)//4, dy1-ROW//2+12, 34, "white", "middle", "Delete", HEAD)

screens(Page(False))
screens(Page(True))
