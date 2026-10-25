# Writes MVG for a game state on screen.png (the same coordinates as render.sh and ui-lib.sh).
#   board-state.py progress      a game in progress: starting numbers, player numbers, pencil marks, one clash
#   board-state.py highlight     the same game after a tap on a 5: all 5s have the highlight color
#   board-state.py unrecognized  the same game immediately after a scribble that the app cannot read: "?" in the cell
#   board-state.py completed     the complete, locked board. The time replaces the difficulty meter.
import os, sys
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
HIGHLIGHT = 5             # the digit that the player taps in "highlight"
UNRECOGNIZED = (4,1)      # the cell with the scribble in "unrecognized"
TIME = "23:41"            # the time used in "completed"

grid = [[int(c) if c != "." else 0 for c in row] for row in GIVENS]
if MODE == "completed":
    grid = [[int(c) for c in row] for row in SOLUTION]
    MARKED = []
else:
    for r, c in ENTERED: grid[r][c] = int(SOLUTION[r][c])
    for (r, c), d in WRONG.items(): grid[r][c] = d

def peers(r, c):
    return [(i, j) for i in range(9) for j in range(9) if (i, j) != (r, c)
            and (i == r or j == c or (i//3 == r//3 and j//3 == c//3))]

# all numbers that have the same number in their row, column or box
conflicts = {(r, c) for r in range(9) for c in range(9)
             if grid[r][c] and any(grid[i][j] == grid[r][c] for i, j in peers(r, c))}

# the app removes pencil marks automatically, so a cell shows only its possible digits
def candidates(r, c):
    used = set(grid[r]) | {grid[i][c] for i in range(9)}
    br, bc = r//3*3, c//3*3
    used |= {grid[i][j] for i in range(br, br+3) for j in range(bc, bc+3)}
    return [d for d in range(1, 10) if d not in used]

out = []
filled = [(r, c) for r in range(9) for c in range(9) if grid[r][c]]
covered = filled + ([UNRECOGNIZED] if MODE == "unrecognized" else [])
# remove the micro-cells below numbers, then draw the board lines again, as render.sh does.
# Backgrounds: first the clash color (light red), then the highlight color (green), then the starting-number gray.
highlighted = {(r, c) for r, c in filled if MODE == "highlight" and grid[r][c] == HIGHLIGHT}
for r, c in covered:
    x, y = X0+c*C, Y0+r*C
    bg = ("#F6C4C4" if (r, c) in conflicts else "#D2EEC2" if (r, c) in highlighted
          else "gray88" if GIVENS[r][c] != "." else "white")
    out.append(f"stroke none fill {bg} rectangle {x},{y} {x+C},{y+C}")
thin = " ".join(f"line {X0+i*C},{Y0} {X0+i*C},{Y0+9*C} line {X0},{Y0+i*C} {X0+9*C},{Y0+i*C}" for i in range(10) if i % 3)
thick = " ".join(f"line {X0+i*C},{Y0} {X0+i*C},{Y0+9*C} line {X0},{Y0+i*C} {X0+9*C},{Y0+i*C}" for i in range(0, 10, 3))
out.append(f"fill none stroke black stroke-width 3 {thin}")
out.append(f"stroke-width 12 stroke-linecap square {thick}")
# numbers: starting numbers black, player numbers dark gray
for r, c in filled:
    x, y = X0+c*C, Y0+r*C
    color = "black" if GIVENS[r][c] != "." else "gray35"
    out.append(f"stroke none fill {color} font '{FONT}' font-size 104 text-anchor middle text {x+C//2},{y+111} '{grid[r][c]}'")
# pencil marks: the possible digits of each cell with marks
for r, c in MARKED:
    assert not grid[r][c]
    x, y = X0+c*C, Y0+r*C
    for d in candidates(r, c):
        cx, cy = x + round(((d-1)%3 + 0.5)*C/3), y + round(((d-1)//3 + 0.5)*C/3)
        out.append(f"stroke none fill gray15 font '{FONT}' font-size 31 text-anchor middle text {cx},{cy+11} '{d}'")

if MODE == "unrecognized":
    r, c = UNRECOGNIZED; x, y = X0+c*C, Y0+r*C
    out.append(f"stroke none fill gray40 font '{FONT}' font-size 104 text-anchor middle text {x+C//2},{y+111} '?'")
if MODE == "completed":
    # the time used replaces the difficulty meter below the board
    dy = Y0 + 9*C + 50
    out.append(f"stroke none fill white rectangle {X0},{dy-30} {X0+9*C},{dy+30}")
    out.append(f"fill black font '{UI_FONT}' font-size 25 text-anchor middle text {X0+9*C//2},{dy+9} 'Completed in {TIME}'")
print("\n".join(out))
