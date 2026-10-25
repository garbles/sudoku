# Makes the analytics screens in the folder argv[1]:
#   analytics.png        summary: completed puzzles and average time, for all puzzles and for each difficulty
#   analytics-games.png  table of completed games (with a difficulty filter). Each game has a delete button.
#   analytics-delete.png the same list after a tap on a bin: a confirmation dialog
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

def chrome(out, title, page=None):
    """'< Back' (returns to the game), the title, and optional page arrows."""
    out.append("fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline 48,37 38,48 48,59")
    text(out, 66, 56, 22, "black", "start", "Back", HEAD)
    text(out, 72, 170, 84, "black", "start", title, HEAD)
    if page:
        n, total = page
        text(out, 702, 1835, 28, "gray50", "middle", f"{n} / {total}")
        prev = "gray80" if n == 1 else "black"; nxt = "gray80" if n == total else "black"
        out.append(f"fill none stroke-width 3 stroke-linecap round stroke-linejoin round stroke {prev} polyline 628,1815 618,1825 628,1835 "
                   f"stroke {nxt} polyline 776,1815 786,1825 776,1835")

def hline(out, y, x0=72, x1=1332, w=1.7):
    out.append(f"stroke none fill black rectangle {x0},{y} {x1},{y+w}")

def render(out, name):
    mvg = os.path.join(OUT, f".{name}.mvg")
    with open(mvg, "w") as f:
        f.write(f"push graphic-context\nscale {SCALE},{SCALE}\n" + "\n".join(out) + "\npop graphic-context\n")
    subprocess.run(["magick", "-size", "1620x2160", "xc:white", "-draw", f"@{mvg}",
                    "-units", "PixelsPerInch", "-density", "229", os.path.join(OUT, name)], check=True)
    os.remove(mvg)

ROW = 97  # 112 device px, the height of a list row on the device

# --- summary
out = []
chrome(out, "Analytics")
total = len(games); avg = sum(g[2] for g in games) / total
for x0, label, value in [(72, "Puzzles completed", str(total)), (716, "Average time", clock(avg))]:
    out.append(f"fill none stroke black stroke-width 1.7 rectangle {x0},250 {x0+616},440")
    text(out, x0+30, 305, 30, "gray40", "start", label)
    text(out, x0+30, 405, 84, "black", "start", value, HEAD)

text(out, 72, 550, 50, "black", "start", "By difficulty", HEAD)
text(out, 72, 615, 30, "gray40", "start", "Difficulty")
text(out, 900, 615, 30, "gray40", "end", "Completed")
text(out, 1332, 615, 30, "gray40", "end", "Average time")
y = 640; hline(out, y)
for mode in range(STOPS):
    times = [g[2] for g in games if g[1] == mode]
    cy = y + ROW//2
    text(out, 72, cy+12, 34, "black", "start", ["Easy", "", "", "", "Insane"][mode])
    meter(out, 230, cy, mode)
    text(out, 900, cy+12, 34, "black", "end", str(len(times)))
    text(out, 1332, cy+12, 34, "black", "end", clock(sum(times)/len(times)) if times else "–")
    y += ROW; hline(out, y)

y += 70
out.append(f"fill none stroke black stroke-width 1.7 rectangle 72,{y} 1332,{y+ROW}")
text(out, 102, y+ROW//2+12, 34, "black", "start", "All completed games", HEAD)
out.append(f"fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round polyline 1292,{y+ROW//2-12} 1304,{y+ROW//2} 1292,{y+ROW//2+12}")
render(out, "analytics.png")

# --- table of completed games, filter "All", newest game first
out = []
pages = (len(games) + PER_PAGE - 1) // PER_PAGE
chrome(out, "Completed games", (1, pages))
# filter: All, then one segment for each difficulty. The selected segment is black, as on the device.
seg_w, sx, sy = 210, 72, 240
for i in range(STOPS + 1):
    x = sx + i*seg_w; selected = i == 0
    out.append(f"fill {'black' if selected else 'white'} stroke black stroke-width 1.7 rectangle {x},{sy} {x+seg_w},{sy+80}")
    if i == 0:
        text(out, x+seg_w//2, sy+52, 32, "white" if selected else "black", "middle", "All", HEAD)
    else:
        meter(out, x+37, sy+40, i-1, step=34)
text(out, 72, 395, 30, "gray40", "start", "Date")
text(out, 560, 395, 30, "gray40", "start", "Difficulty")
text(out, 1110, 395, 30, "gray40", "end", "Time")
y = 420; hline(out, y)
for when, mode, secs in games[:PER_PAGE]:
    cy = y + ROW//2
    text(out, 72, cy+12, 34, "black", "start", when.strftime("%-d %b %Y, %H:%M"))
    meter(out, 560, cy, mode)
    text(out, 1110, cy+12, 34, "black", "end", clock(secs))
    # delete: a bin (lid, handle, body with two lines)
    bx = 1290
    out.append(f"fill none stroke black stroke-width 2.6 stroke-linecap round stroke-linejoin round "
               f"line {bx-14},{cy-11} {bx+14},{cy-11} polyline {bx-5},{cy-11} {bx-5},{cy-15} {bx+5},{cy-15} {bx+5},{cy-11} "
               f"polyline {bx-10},{cy-11} {bx-8},{cy+15} {bx+8},{cy+15} {bx+10},{cy-11} "
               f"line {bx-3},{cy-5} {bx-3},{cy+9} line {bx+3},{cy-5} {bx+3},{cy+9}")
    y += ROW; hline(out, y)
render(out, "analytics-games.png")

# --- delete confirmation for the second game in the list
when, mode, secs = games[1]
out.append("stroke none fill rgba(255,255,255,0.6) rectangle 0,0 1404,1872")
dx0, dx1, dy0, dy1 = 252, 1152, 760, 1110
out.append(f"fill white stroke black stroke-width 1.7 rectangle {dx0},{dy0} {dx1},{dy1}")
text(out, dx0+40, dy0+75, 44, "black", "start", "Delete this game?", HEAD)
text(out, dx0+40, dy0+140, 34, "gray25", "start", f"{when.strftime('%-d %b %Y, %H:%M')}, completed in {clock(secs)}.")
text(out, dx0+40, dy0+190, 34, "gray25", "start", "You cannot undo this action.")
hline(out, dy1-ROW, dx0, dx1)
out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {dx1},{dy1}")
out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {(dx0+dx1)//2+1.7},{dy1}")
text(out, (dx0*3+dx1)//4, dy1-ROW//2+12, 34, "black", "middle", "Cancel", HEAD)
text(out, (dx0+dx1*3)//4, dy1-ROW//2+12, 34, "white", "middle", "Delete", HEAD)
render(out, "analytics-delete.png")
