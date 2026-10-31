#!/usr/bin/env bash
# The performance bench on the computer (refer to src/bench.rs and AGENTS.md):
#
#   scripts/bench.sh [--runs N] [--baseline FILE] [--save] [--note TEXT] [-- BENCH OPTIONS]
#
# It builds the release bench, does one warm-up run and N runs (default 5), and keeps the median of
# each value. It compares the result with the baseline (default bench/baseline-computer.json) and adds
# one line to bench/history.jsonl. If an output hash is not the same as in the baseline, the
# optimization changed the output: the script fails. --save writes the result as the new baseline.
#
# On the tablet: `sudoku --bench /tmp/bench.json`, then copy the file and compare it with
# `scripts/bench.sh --compare FILE --baseline bench/baseline-tablet.json`.
# In the Linux container (musl, as the tablet build): `--baseline bench/baseline-linux.json`.
set -euo pipefail
cd "$(dirname "$0")/.."

runs=5
baseline=bench/baseline-computer.json
save=0
note=""
compare=""
while [ $# -gt 0 ]; do
  case "$1" in
    --runs) runs=$2; shift 2 ;;
    --baseline) baseline=$2; shift 2 ;;
    --save) save=1; shift ;;
    --note) note=$2; shift 2 ;;
    --compare) compare=$2; shift 2 ;;
    --) shift; break ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
if [ -n "$compare" ]; then
  cp "$compare" "$out/run-1.json"
else
  cargo build --release --quiet --bin bench
  bench="${CARGO_TARGET_DIR:-target}/release/bench"
  "$bench" "$out/warm.json" "$@" 2>/dev/null
  for i in $(seq 1 "$runs"); do
    "$bench" "$out/run-$i.json" "$@" 2>/dev/null
  done
fi

python3 - "$out" "$baseline" "$save" "$note" <<'PY'
import glob, json, os, statistics, subprocess, sys, time
out, baseline_path, save, note = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4]
runs = [json.load(open(f)) for f in sorted(glob.glob(os.path.join(out, "run-*.json")))]
hashes = runs[0]["hashes"]
for r in runs[1:]:
    if r["hashes"] != hashes:
        sys.exit(f"the output hashes are not the same in each run: {r['hashes']} {hashes}")
phases = {}
for name in runs[0]["phases"]:
    phases[name] = {k: statistics.median(r["phases"][name][k] for r in runs) for k in runs[0]["phases"][name]}
result = {"phases": phases, "hashes": hashes, "runs": len(runs)}

base = json.load(open(baseline_path)) if os.path.exists(baseline_path) else None
print(f"{'phase':22} {'mean us':>10} {'base':>10} {'change':>8}   {'p95 us':>10} {'base':>10} {'change':>8}")
def pct(a, b):
    return f"{(a - b) / b * 100:+7.1f}%" if b else "       -"
for name, v in phases.items():
    b = base["phases"].get(name) if base else None
    bm = b["mean_us"] if b else 0
    bp = b["p95_us"] if b else 0
    print(f"{name:22} {v['mean_us']:10.1f} {bm:10.1f} {pct(v['mean_us'], bm)}   {v['p95_us']:10.1f} {bp:10.1f} {pct(v['p95_us'], bp)}")
bad = []
if base:
    for k, h in hashes.items():
        if base["hashes"].get(k) != h:
            bad.append(f"{k}: {base['hashes'].get(k)} -> {h}")
for b in bad:
    print(f"OUTPUT CHANGED {b}")

sha = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
dirty = subprocess.run(["git", "status", "--porcelain", "--", "src"], capture_output=True, text=True).stdout.strip() != ""
line = {"time": int(time.time()), "git": sha + ("+" if dirty else ""), "baseline": os.path.basename(baseline_path), "note": note,
        "mean_us": {k: v["mean_us"] for k, v in phases.items()}, "p95_us": {k: v["p95_us"] for k, v in phases.items()},
        "output_changed": bool(bad)}
with open("bench/history.jsonl", "a") as f:
    f.write(json.dumps(line) + "\n")
if save:
    with open(baseline_path, "w") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    print(f"saved {baseline_path}")
if bad and not save:
    sys.exit(1)
PY
