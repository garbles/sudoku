# Makes the analytics screens in the folder argv[1]:
#   analytics.png        summary: completed puzzles and average time, for all puzzles and for each difficulty
#   analytics-games.png  table of completed games (with a difficulty filter). Each game has a delete button.
#   analytics-delete.png the same list after a tap on a bin: a confirmation dialog
#   analytics-delete-all.png the summary after a tap on "Delete all games": a confirmation dialog
#   licenses.png         the first page of the Licenses list (REQUIREMENTS.md §9.4)
#   licenses-text.png    the first page of the license text of the UJI data
#   update-restart.png   settings-restart.png (toolbar.sh) with the dialog "Restart the interface?": after
#                        an update, the program runs as an external app and its folder has the
#                        frontend files (REQUIREMENTS.md §9.2)
# and the same four screens for landscape (analytics-landscape.png, analytics-games-landscape.png,
# analytics-delete-landscape.png, analytics-delete-all-landscape.png): the page turns (1872x1404 units on the Paper
# Pro), with the same scale. The positions are the same as in src/layout.rs.
# All screens use one list of sample games (the same data as the SQLite results table). Thus the
# summary and the table agree. The layout uses the units of the manual (1404x1872 on the Paper Pro), with the scale
# PAGE_SCALE of the device (devices/DEVICE.conf, refer to device-lib.sh) to the screen.
import datetime, json, os, random, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1]
FONT = os.path.join(HERE, "fonts", "rmsans-400.ttf")
HEAD = os.path.join(HERE, "fonts", "rmsans-500.ttf")

def device_values(name):
    """The values of the tablet model `name` from devices/<name>.conf (refer to device-lib.sh)."""
    values = {}
    with open(os.path.join(HERE, "devices", name + ".conf")) as f:
        for line in f:
            line = line.split("#")[0].strip()
            if line:
                key, value = line.split("=")
                values[key] = value
    return values

def page_size(values):
    """The page in layout units (portrait): the screen over the scale, rounded (the same as device-lib.sh)."""
    scale = float(values["PAGE_SCALE"])
    return int(int(values["PHYS_W"]) / scale + 0.5), int(int(values["PHYS_H"]) / scale + 0.5)

DEVICE = device_values(os.environ.get("DEVICE", "pro"))   # the tablet model (default: pro)
PHYS_W, PHYS_H, DPI = int(DEVICE["PHYS_W"]), int(DEVICE["PHYS_H"]), DEVICE["DPI"]
SCALE = float(DEVICE["PAGE_SCALE"])
PAGE_W, PAGE_H = page_size(DEVICE)
# The page of the Paper Pro (1404x1872): the fixed row counts of the lists and of "By difficulty" fill it.
PRO_PAGE_W, PRO_PAGE_H = page_size(device_values("pro"))
STOPS = 7
TEXT_GRAY = "gray25"   # the secondary text (ui-lib.sh TEXT_GRAY, #404040)

# --- sample results: (finished_at, mode 0-4, seconds). The modes 5 and 6 have no sample results.
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
    s = s.replace("\\", "\\\\").replace('"', '\\"')
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

# The page geometry (src/layout.rs): on the Paper Pro, portrait 1404x1872 units, landscape 1872x1404 units.
class Page:
    def __init__(self, landscape):
        self.landscape = landscape
        self.w, self.h = (PAGE_H, PAGE_W) if landscape else (PAGE_W, PAGE_H)
        self.size = f"{PHYS_H}x{PHYS_W}" if landscape else f"{PHYS_W}x{PHYS_H}"
        self.right = self.w - 72                      # 1332 in portrait
        self.box_w = (self.w - 172) / 2               # 616 in portrait
        self.count_x = self.right - 432               # 900 in portrait
        self.seg_w = (self.w - 144) / (STOPS + 1)     # 157.5 in portrait
        # The height more than the page of the Paper Pro (src/layout.rs extra_page_h): 624 on the Move in portrait.
        self.extra = max(0, round(self.h - (PRO_PAGE_W if landscape else PRO_PAGE_H)))
        # the rows of "By difficulty": 7 rows fit in landscape; a taller page divides its extra height between them
        self.summary_row = (76 if landscape else ROW) + self.extra // STOPS
        self.per_page = (6 if landscape else 12) + self.extra // ROW   # the games list: one table across the full width
        self.licenses_rows = (9 if landscape else 14) + self.extra // ROW
        self.license_lines = (27 if landscape else 40) + self.extra // LICENSE_LINE
        self.col_w = self.w - 144
        self.offsets = (488, self.col_w - 222, self.col_w - 42)   # meter, end of time, bin (1038, 1218 in portrait)
        self.dialog = (self.w / 2 - 450, self.h / 2 - 176, self.w / 2 + 450, self.h / 2 + 174)
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
        text(out, cx, ny, 28, "gray25", "middle", f"{k} / {total}")
        prev = "gray80" if k == 1 else "black"; nxt = "gray80" if k == total else "black"
        out.append(f"fill none stroke-width 3 stroke-linecap round stroke-linejoin round stroke {prev} polyline {n(cx-74)},{n(ny-20)} {n(cx-84)},{n(ny-10)} {n(cx-74)},{ny} "
                   f"stroke {nxt} polyline {n(cx+74)},{n(ny-20)} {n(cx+84)},{n(ny-10)} {n(cx+74)},{ny}")

def hline(out, y, x0, x1, w=1.7):
    out.append(f"stroke none fill black rectangle {n(x0)},{n(y)} {n(x1)},{n(y+w)}")

def render(out, P, name, base=None):
    """Draws the MVG on a white page, or on the image `base` of OUT."""
    mvg = os.path.join(OUT, f".{name}.mvg")
    with open(mvg, "w") as f:
        f.write(f"push graphic-context\nscale {SCALE},{SCALE}\n" + "\n".join(out) + "\npop graphic-context\n")
    start = [os.path.join(OUT, base)] if base else ["-size", P.size, "xc:white"]
    subprocess.run(["magick", *start, "-draw", f"@{mvg}",
                    "-units", "PixelsPerInch", "-density", DPI, os.path.join(OUT, P.suffix(name))], check=True)
    os.remove(mvg)

ROW = 97  # 112 device px, the height of a list row on the device
LICENSE_LINE = 36  # the distance between two lines of license text (src/layout.rs, Device::licenses)
FILTER_STEP = 18  # the distance between the stops of the small meters in the filter segments

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
        text(out, x0+30, 305, 30, TEXT_GRAY, "start", label)
        text(out, x0+30, 405, 84, "black", "start", value, HEAD)

    text(out, 72, 550, 50, "black", "start", "By difficulty", HEAD)
    text(out, 72, 615, 30, TEXT_GRAY, "start", "Difficulty")
    text(out, n(P.count_x), 615, 30, TEXT_GRAY, "end", "Completed")
    text(out, n(P.right), 615, 30, TEXT_GRAY, "end", "Average time")
    y = 640; hline(out, y, 72, P.right)
    for mode in range(STOPS):
        times = [g[2] for g in games if g[1] == mode]
        cy = y + P.summary_row//2
        text(out, 72, cy+12, 34, "black", "start", {0: "Easy", STOPS-1: "Hard"}.get(mode, ""))
        meter(out, 230, cy, mode)
        text(out, n(P.count_x), cy+12, 34, "black", "end", str(len(times)))
        text(out, n(P.right), cy+12, 34, "black", "end", clock(sum(times)/len(times)) if times else "–")
        y += P.summary_row; hline(out, y, 72, P.right)

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
    pages = (len(games) + P.per_page - 1) // P.per_page
    chrome(out, P, "Completed games", (1, pages))
    # filter: All, then one segment for each difficulty. The selected segment is black, as on the device.
    seg_w, sx, sy = P.seg_w, 72, 240
    for i in range(STOPS + 1):
        x = n(sx + i*seg_w); selected = i == 0
        out.append(f"fill {'black' if selected else 'white'} stroke black stroke-width 1.7 rectangle {x},{sy} {n(x+seg_w)},{sy+80}")
        if i == 0:
            text(out, n(x+seg_w/2), sy+52, 32, "white" if selected else "black", "middle", "All", HEAD)
        else:
            meter(out, n(x+(seg_w-(STOPS-1)*FILTER_STEP)/2), sy+40, i-1, step=FILTER_STEP)
    md, td, bd = P.offsets
    rows = games[:P.per_page]
    cx0 = 72
    text(out, cx0, 395, 30, TEXT_GRAY, "start", "Date")
    text(out, n(cx0+md), 395, 30, TEXT_GRAY, "start", "Difficulty")
    text(out, n(cx0+td), 395, 30, TEXT_GRAY, "end", "Time")
    hline(out, 420, cx0, cx0 + P.col_w)
    for i, (when, mode, secs) in enumerate(rows):
        y = 420 + i*ROW
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

DELETE_ALL_W = 400   # the width of "Delete all games" (SummaryButtonMetrics::delete_all_width in src/layout.rs)

def dialog(out, P, title, line, line2="You cannot undo this action.", buttons=("Cancel", "Delete")):
    """A dialog at the center of the page, on the pale screen. The right button is white on black."""
    out.append(f"stroke none fill rgba(255,255,255,0.6) rectangle 0,0 {P.w},{P.h}")
    dx0, dy0, dx1, dy1 = (n(v) for v in P.dialog)
    out.append(f"fill white stroke black stroke-width 1.7 rectangle {dx0},{dy0} {dx1},{dy1}")
    text(out, dx0+40, dy0+75, 44, "black", "start", title, HEAD)
    text(out, dx0+40, dy0+140, 34, "gray25", "start", line)
    text(out, dx0+40, dy0+190, 34, "gray25", "start", line2)
    hline(out, dy1-ROW, dx0, dx1)
    out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {dx1},{dy1}")
    out.append(f"stroke none fill black rectangle {(dx0+dx1)//2},{dy1-ROW} {(dx0+dx1)//2+1.7},{dy1}")
    text(out, (dx0*3+dx1)//4, dy1-ROW//2+12, 34, "black", "middle", buttons[0], HEAD)
    text(out, (dx0+dx1*3)//4, dy1-ROW//2+12, 34, "white", "middle", buttons[1], HEAD)

# --- the Licenses screens (REQUIREMENTS.md §9.4, src/licenses.rs). The data is assets/licenses.json.
with open(os.path.join(HERE, "..", "assets", "licenses.json"), encoding="utf-8") as f:
    LICENSES = json.load(f)
LICENSES_Y = 300          # the top of the list (src/layout.rs, Device::licenses)
WRAP = 80                 # the maximum characters in a line of license text (src/licenses.rs)

def wrap(line, width=WRAP):
    """The same cuts as licenses::wrap: at the last space before the limit, or at the limit."""
    parts, rest = [], line
    while len(rest) > width:
        if rest[width] == " ":
            cut = width
        else:
            i = rest[:width].rfind(" ")
            cut = i if i > 0 else width
        parts.append(rest[:cut].rstrip()); rest = rest[cut:].lstrip()
    if rest or not parts:
        parts.append(rest)
    return parts

def license_lines(entry):
    """The lines of licenses::lines: (kind, text) with kind "head", "body" or "blank"."""
    out = []
    for k, f in enumerate(entry["files"]):
        if k > 0:
            out.append(("blank", ""))
        out += [("head", f["name"]), ("blank", "")]
        for line in LICENSES["texts"][f["text"]].split("\n"):
            out += [("blank", "")] if not line else [("body", part) for part in wrap(line)]
    return out

def chevron(out, cx, cy):
    """The › of render::Icon::OpenList."""
    out.append(f"fill none stroke black stroke-width 3 stroke-linecap round stroke-linejoin round "
               f"polyline {n(cx-6)},{n(cy-12)} {n(cx+6)},{n(cy)} {n(cx-6)},{n(cy+12)}")

def licenses(P):
    crates = LICENSES["crates"]
    rows = P.licenses_rows
    x, w = 72, P.col_w
    out = []
    chrome(out, P, "Licenses", (1, (len(crates) + rows - 1) // rows))
    text(out, x, 270, 30, TEXT_GRAY, "start", "The app contains these open source components.")
    hline(out, LICENSES_Y, x, x + w)
    for i, c in enumerate(crates[:rows]):
        y = LICENSES_Y + i*ROW
        base = n(y + ROW/2 + 11)
        text(out, x, base, 32, "black", "start", c["name"], HEAD)
        text(out, n(x + w - 60), base, 30, TEXT_GRAY, "end", c["version"])
        chevron(out, x + w - 15, y + ROW/2)
        hline(out, y + ROW, x, x + w)
    render(out, P, "licenses.png")

    # The first page of the text of the UJI data.
    c = next(c for c in crates if c["name"] == "UJI Pen Characters")
    lines, per_page = license_lines(c), P.license_lines
    out = []
    chrome(out, P, c["name"], (1, (len(lines) + per_page - 1) // per_page))
    text(out, x, 232, 30, TEXT_GRAY, "start", f"Version {c['version']} · {c['license'].replace(' OR ', ' / ')}")
    hline(out, 260, x, x + w)
    for i, (kind, s) in enumerate(lines[:per_page]):
        y = 320 + i*LICENSE_LINE
        if kind == "head":
            text(out, x, y, 28, "black", "start", s, HEAD)
        elif kind == "body":
            text(out, x, y, 25, "black", "start", s)
    render(out, P, "licenses-text.png")

screens(Page(False))
screens(Page(True))
licenses(Page(False))
licenses(Page(True))

# --- the dialog "Restart the interface?" over the Settings panel (src/render.rs): after an update, the
# program runs as an external app and its folder has the frontend files
out = []
dialog(out, Page(False), "Restart the interface?", "The update is not complete. To complete it,",
       "the reMarkable interface must start again.", ("Later", "Restart"))
render(out, Page(False), "update-restart.png", base="settings-restart.png")
