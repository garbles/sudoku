# Writes MVG for a game state on screen.png (the same coordinates as render.sh and ui-lib.sh).
#   board-state.py progress      a game in progress: starting numbers, player numbers, pencil marks, one clash
#   board-state.py highlight     the same game after a tap on a 5: all 5s have the highlight color
#   board-state.py unrecognized  the same game immediately after a scribble that the app cannot read: "?" in the cell
#   board-state.py invalid-mark  the same game immediately after a pen tap on a pencil mark that a peer refuses: "x" in the micro-cell
#   board-state.py completed-group  a game with no clash and a complete box, column and row in the completed color
#   board-state.py completed     the complete, locked board with the normal colors. The result card replaces the difficulty meter.
#   board-state.py mini STATE X Y CELL  the small board of a checkpoint (MINI_STATES) with its first cell at X,Y
#   board-state.py mini-count STATE     the quantity of filled cells of that checkpoint
import os, subprocess, sys
C = 147; X0, Y0 = (1620 - 9*C)//2, (2160 - 9*C)//2   # must agree with ui-lib.sh
HERE = os.path.dirname(os.path.abspath(__file__))
FONT = os.path.join(HERE, "fonts", "rmsans-400.ttf")     # reMarkable Sans Regular
UI_FONT = os.path.join(HERE, "fonts", "rmsans-500.ttf")  # reMarkable Sans Medium (UI text)
MODE = sys.argv[1] if len(sys.argv) > 1 else "progress"

SOLUTION = ["534678912","672195348","198342567","859761423","426853791",
            "713924856","961537284","287419635","345286179"]
GIVENS = ["53..7....","6..195...",".98....6.","8...6...3","4..8.3..1",
          "7...2...6",".6....28.","...419..5","....8..79"]
# cells with player numbers (all correct), and cells with pencil marks
ENTERED = [(0,2),(0,3),(1,1),(1,2),(2,0),(3,1),(4,4),(5,4),(6,0),(7,0),(7,8),(8,6)]
MARKED = [(0,5),(0,6),(1,6),(2,3),(2,4),(3,2),(3,5),(4,2),(5,2),(6,3),(6,5),(8,1),(8,2),(4,6)]
# a mistake: the player wrote a 9 here, but column 1 has a 9 already
WRONG = {(5,1): 9}
# "completed-group" has no mistake and these numbers more. Then box 0, column 4 and row 7 are complete.
GROUP_ENTERED = [(2,4),(6,4),(7,1),(7,2),(7,6),(7,7)]
# The colors of a cell (the defaults of the panel, the same values as ui-lib.sh) and the digits of the player.
STARTING_BG, CLASH_BG, HIGHLIGHT_BG, COMPLETED_BG = "#AEAEAE", "#FF8E8E", "#6FCFE6", "#48D630"
PLAYER_DIGIT = "#1E1E1E"
HIGHLIGHT = 5             # the digit that the player taps in "highlight"
UNRECOGNIZED = (4,1)      # the cell with the scribble in "unrecognized"
INVALID_MARK = (1,6,6)    # the row, the column and the digit of the tap in "invalid-mark". (1,0) and (2,7) have a 6.
X_HALF = 10               # each line of the x goes from -10 px to +10 px on the two axes around the center
X_WIDTH = 3               # the line width of the x
# The small boards of the Checkpoints panel: the player numbers, the mistakes and the cells with pencil marks.
MINI_STATES = {
    "current": (ENTERED, WRONG, MARKED),                        # the same as the game in progress
    "group": (ENTERED + GROUP_ENTERED, {}, MARKED),             # the same numbers as "completed-group"
    "clean": (ENTERED, {}, MARKED),                             # the game in progress before the mistake
    "mid": (ENTERED[:6], {}, MARKED[:8]),
    "early": (ENTERED[:2], {}, MARKED[:4]),
}
MINI_FONT_SIZE = 14       # the digits of the small board
MINI_BASELINE = 14        # the baseline of a digit, from the top of its cell

grid = [[int(c) if c != "." else 0 for c in row] for row in GIVENS]
if MODE == "completed":
    grid = [[int(c) for c in row] for row in SOLUTION]
    MARKED = []
else:
    if MODE == "completed-group":
        ENTERED, WRONG = ENTERED + GROUP_ENTERED, {}
    for r, c in ENTERED: grid[r][c] = int(SOLUTION[r][c])
    for (r, c), d in WRONG.items(): grid[r][c] = d
    MARKED = [(r, c) for r, c in MARKED if not grid[r][c]]

def peers(r, c):
    return [(i, j) for i in range(9) for j in range(9) if (i, j) != (r, c)
            and (i == r or j == c or (i//3 == r//3 and j//3 == c//3))]

# all numbers that have the same number in their row, column or box
def find_conflicts(b):
    return {(r, c) for r in range(9) for c in range(9)
            if b[r][c] and any(b[i][j] == b[r][c] for i, j in peers(r, c))}
conflicts = find_conflicts(grid)

# A row, a column or a box is complete when all nine cells are filled and none of its cells clashes. A clash with a
# cell outside the group also stops the group. A completed game shows no completed color.
GROUPS = ([[(r, c) for c in range(9)] for r in range(9)] + [[(r, c) for r in range(9)] for c in range(9)]
          + [[(br+i, bc+j) for i in range(3) for j in range(3)] for br in (0, 3, 6) for bc in (0, 3, 6)])
def find_complete(b, clashes):
    return [g for g in GROUPS if all(b[r][c] for r, c in g) and not any(p in clashes for p in g)]
complete_groups = [] if MODE == "completed" else find_complete(grid, conflicts)
complete = {p for g in complete_groups for p in g}
if MODE == "completed-group":
    assert not conflicts and len(complete_groups) == 3
else:
    assert not complete_groups   # the sample game has a full box with a clash outside the box: no completed color

# the app hides pencil marks automatically, so a cell shows only its possible digits
def candidates(r, c):
    used = set(grid[r]) | {grid[i][c] for i in range(9)}
    br, bc = r//3*3, c//3*3
    used |= {grid[i][j] for i in range(br, br+3) for j in range(bc, bc+3)}
    return [d for d in range(1, 10) if d not in used]

# the anchor point of the pencil mark of digit d in the cell at x,y: 24, 74 or 122 px on each axis
def mark_anchor(x, y, d):
    return x + round(((d-1)%3 + 0.5)*C/3), y + round(((d-1)//3 + 0.5)*C/3)

# The small board of a checkpoint. Cell (r, c) has its top left pixel at (X + c*CELL, Y + r*CELL). Each line of 1 px
# is on the first pixel of the cell after it. Each line of 2 px is on the last pixel of the cell before it and the
# first pixel of the cell after it. Thus the board with its outer lines is 2 px more than 9 cells, 1 px on each side.
def mini_grid(state):
    entered, wrong, _ = MINI_STATES[state]
    b = [[int(c) if c != "." else 0 for c in row] for row in GIVENS]
    for r, c in entered: b[r][c] = int(SOLUTION[r][c])
    for (r, c), d in wrong.items(): b[r][c] = d
    return b

def mini_board(state, bx, by, cell):
    marked = MINI_STATES[state][2]
    b = mini_grid(state)
    clashes = find_conflicts(b)
    done = {p for grp in find_complete(b, clashes) for p in grp}
    side = 9*cell
    m = []
    for r in range(9):
        for c in range(9):
            if not b[r][c]: continue
            x, y = bx+c*cell, by+r*cell
            bg = (CLASH_BG if (r, c) in clashes else COMPLETED_BG if (r, c) in done
                  else STARTING_BG if GIVENS[r][c] != "." else "white")
            m.append(f"stroke none fill {bg} rectangle {x},{y} {x+cell-1},{y+cell-1}")
    # one dot of 2x2 px for each visible pencil mark
    for r, c in marked:
        if b[r][c]: continue
        used = {b[i][j] for i, j in peers(r, c)}
        x, y = bx+c*cell, by+r*cell
        for d in range(1, 10):
            if d in used: continue
            dx, dy = x+3+5*((d-1) % 3), y+3+5*((d-1)//3)
            m.append(f"stroke none fill gray25 rectangle {dx},{dy} {dx+1},{dy+1}")
    for i in range(1, 9):
        if i % 3:
            a = i*cell
            m.append(f"stroke none fill gray60 rectangle {bx+a},{by} {bx+a},{by+side-1} rectangle {bx},{by+a} {bx+side-1},{by+a}")
    for i in range(0, 10, 3):
        a = i*cell
        m.append(f"stroke none fill black rectangle {bx+a-1},{by-1} {bx+a},{by+side} rectangle {bx-1},{by+a-1} {bx+side},{by+a}")
    for r in range(9):
        for c in range(9):
            if not b[r][c]: continue
            x, y = bx+c*cell, by+r*cell
            color = "black" if GIVENS[r][c] != "." else PLAYER_DIGIT
            m.append(f"stroke none fill {color} font '{FONT}' font-size {MINI_FONT_SIZE} text-anchor middle "
                     f"text {x+cell/2},{y+MINI_BASELINE} '{b[r][c]}'")
    return m

if MODE == "mini":
    print("\n".join(mini_board(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]))))
    sys.exit(0)
if MODE == "mini-count":
    print(sum(1 for row in mini_grid(sys.argv[2]) for d in row if d))   # all filled cells, also the starting numbers
    sys.exit(0)

out = []
filled =[(r, c) for r in range(9) for c in range(9) if grid[r][c]]
covered = filled + ([UNRECOGNIZED] if MODE == "unrecognized" else [])
# remove the micro-cells below numbers, then draw the board lines again, as render.sh does.
# Backgrounds: the first rule that applies: the clash color, the highlight color, the completed color, the
# starting-number color, white.
highlighted = {(r, c) for r, c in filled if MODE == "highlight" and grid[r][c] == HIGHLIGHT}
for r, c in covered:
    x, y = X0+c*C, Y0+r*C
    bg = (CLASH_BG if (r, c) in conflicts else HIGHLIGHT_BG if (r, c) in highlighted
          else COMPLETED_BG if (r, c) in complete
          else STARTING_BG if GIVENS[r][c] != "." else "white")
    out.append(f"stroke none fill {bg} rectangle {x},{y} {x+C},{y+C}")
thin = " ".join(f"line {X0+i*C},{Y0} {X0+i*C},{Y0+9*C} line {X0},{Y0+i*C} {X0+9*C},{Y0+i*C}" for i in range(10) if i % 3)
thick = " ".join(f"line {X0+i*C},{Y0} {X0+i*C},{Y0+9*C} line {X0},{Y0+i*C} {X0+9*C},{Y0+i*C}" for i in range(0, 10, 3))
# The board lines have no anti-aliasing, as render.sh: the pixels whose centers are inside the stroke are black.
out.append(f"stroke-antialias 0 fill none stroke black stroke-width 3 {thin}")
out.append(f"stroke-width 12 stroke-linecap square {thick} stroke-antialias 1")
# numbers: starting numbers black, player numbers #1E1E1E
for r, c in filled:
    x, y = X0+c*C, Y0+r*C
    color = "black" if GIVENS[r][c] != "." else PLAYER_DIGIT
    out.append(f"stroke none fill {color} font '{FONT}' font-size 104 text-anchor middle text {x+C//2},{y+111} '{grid[r][c]}'")
# pencil marks: the possible digits of each cell with marks. The app draws the marks with no anti-aliasing
# (REQUIREMENTS.md §15): each pixel is gray15 or the pixel under it. The other texts of the image keep their
# anti-aliasing, thus the primitive goes back to 1 after the marks.
out.append("text-antialias 0")
for r, c in MARKED:
    assert not grid[r][c]
    x, y = X0+c*C, Y0+r*C
    for d in candidates(r, c):
        cx, cy = mark_anchor(x, y, d)
        out.append(f"stroke none fill gray15 font '{FONT}' font-size 31 text-anchor middle text {cx},{cy+11} '{d}'")
out.append("text-antialias 1")

if MODE == "unrecognized":
    r, c = UNRECOGNIZED; x, y = X0+c*C, Y0+r*C
    out.append(f"stroke none fill gray25 font '{FONT}' font-size 104 text-anchor middle text {x+C//2},{y+111} '?'")
if MODE == "invalid-mark":
    # only the micro-cell changes: the other marks and the micro-cell lines of the cell stay
    r, c, d = INVALID_MARK
    assert not grid[r][c] and d not in candidates(r, c)
    # The app draws the x with no anti-aliasing, as the marks: each pixel is gray15 or the pixel under it.
    cx, cy = mark_anchor(X0+c*C, Y0+r*C, d)
    h = X_HALF
    out.append(f"stroke-antialias 0 fill none stroke gray15 stroke-width {X_WIDTH} stroke-linecap round "
               f"line {cx-h},{cy-h} {cx+h},{cy+h} line {cx-h},{cy+h} {cx+h},{cy-h} stroke-antialias 1")
if MODE == "completed":
    # the result card replaces the difficulty meter below the board (ui-lib.sh result_card, mode 2 = LEVEL)
    dy = Y0 + 9*C + 50
    out.append(f"stroke none fill white rectangle {X0},{dy-30} {X0+9*C},{dy+30}")
    card = subprocess.run(["sh", "-c", '. "$0"; result_card portrait $LEVEL', os.path.join(HERE, "ui-lib.sh")],
                          check=True, capture_output=True, text=True).stdout
    out.append(card)
print("\n".join(out))
