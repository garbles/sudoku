#!/usr/bin/env python3
"""Make an HTML page that shows the rules of INVARIANTS.md (the visualizer).

The page shows each rule with its section, its source, the rules that it refers to, the rules that
refer to it and the files that give its ID (tests/ and src/). It compares the working tree with an
earlier version of INVARIANTS.md and shows the rules that are new, changed, moved or removed. The
page has the earlier versions (the commits that changed the file, and the tags), thus you can
select a different base in the page.

Default base:
- On a branch that is not main: the merge base with main (the change of the branch).
- On main, when INVARIANTS.md has changes that are not committed: HEAD.
- Else: the version before the last commit that changed INVARIANTS.md.

Usage:
  python3 scripts/invariants.py [--base REF] [--out FILE] [--history N] [--open]
  python3 scripts/invariants.py --check [--file FILE]   (examine the format only; no git; exit 1 on a problem)

Only the Python standard library. The format of INVARIANTS.md is in the comment at its start.
"""

import argparse
import json
import re
import subprocess
import sys
import webbrowser
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = "INVARIANTS.md"
ID_RE = re.compile(r"\b([A-Z]{2,4}-\d+)\b")
ROW_RE = re.compile(r"^\|\s*([A-Z]{2,4}-\d+)\s*\|")
HEADING_RE = re.compile(r"^(#{2,3})\s+(\d+(?:\.\d+)?)\.?\s+(.*)$")
TERM_RE = re.compile(r"^- \*\*(.+?)\*\*:\s*(.*)$")


def git(*args, check=True):
    r = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout if r.returncode == 0 else None


def split_row(line):
    """Divide a table row into cells. A "|" in a code span or after "\\" is not a border."""
    cells, cur, code = [], [], False
    i = 0
    body = line.strip()
    if body.startswith("|"):
        body = body[1:]
    if body.endswith("|"):
        body = body[:-1]
    while i < len(body):
        ch = body[i]
        if ch == "\\" and i + 1 < len(body) and body[i + 1] == "|":
            cur.append("|")
            i += 2
            continue
        if ch == "`":
            code = not code
        if ch == "|" and not code:
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(ch)
        i += 1
    cells.append("".join(cur).strip())
    return cells


def between(lines, start, end):
    """The lines between two marker comments, or None if the markers are not there."""
    try:
        a = next(i for i, l in enumerate(lines) if l.strip() == f"<!-- invariants:{start} -->")
        b = next(i for i, l in enumerate(lines) if i > a and l.strip() == f"<!-- invariants:{end} -->")
    except StopIteration:
        return None
    return lines[a + 1 : b]


# The grammar of the state model (one line at a time; the same text always gives the same result):
#   type header      Name                                   (a capital letter, then letters)
#   alias            Name = definition   [note]
#   field line       "  " names [op domain]   [note]        (op: ∈ : ⊆ ≥; names: a, b[r][c], ...)
#   continuation     20 or more spaces, then text           (more note of the item above)
#   blank line       ends nothing; it only separates
# A note is a group in parentheses at the end of the line, after two or more spaces.
TYPE_RE = re.compile(r"^([A-Z][A-Za-z]*)$")
ALIAS_RE = re.compile(r"^([A-Z][A-Za-z]*) = (.+)$")
NAME = r"[a-z_][a-z0-9_]*(?:\[r\]\[c\])?"
FIELD_RE = re.compile(rf"^  (?P<names>{NAME}(?:, {NAME})*)(?:\s+(?P<op>∈|:|⊆|≥)\s+(?P<domain>\S.*?))?\s*$")
CONT_INDENT = 20


def split_note(text):
    """Divide a line into (body, note). The note is the last group in parentheses, at the end of the
    line, after two or more spaces."""
    if not text.endswith(")"):
        return text.rstrip(), ""
    depth = 0
    for i in range(len(text) - 1, -1, -1):
        if text[i] == ")":
            depth += 1
        elif text[i] == "(":
            depth -= 1
            if depth == 0:
                if i >= 2 and text[i - 2 : i] == "  ":
                    return text[:i].rstrip(), text[i + 1 : -1].strip()
                return text.rstrip(), ""
    return text.rstrip(), ""


def top_level(text):
    """The text with the parts in braces and in parentheses replaced by spaces."""
    out, depth = [], 0
    for ch in text:
        if ch in "{(":
            depth += 1
        out.append(ch if depth == 0 else " ")
        if ch in "})" and depth:
            depth -= 1
    return "".join(out)


def type_refs(domain, names):
    """The types that a domain refers to: the type names out of braces and parentheses."""
    return sorted({w for w in re.findall(r"\b[A-Z][A-Za-z]*\b", top_level(domain)) if w in names})


def parse_model(lines, first_line=1):
    """Read the lines of the state model. Gives the types and the errors (with the line numbers of
    the file)."""
    types, errors = [], []
    cur_type, last = None, None
    for k, raw in enumerate(lines):
        n = first_line + k
        line = raw.rstrip()
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip(" "))
        if indent >= CONT_INDENT:
            if last is None:
                errors.append(f"line {n}: a continuation line with no item above it")
            else:
                last["more"] = (last["more"] + " " + line.strip()).strip()
            continue
        if indent == 0:
            body, note = split_note(line)
            a = ALIAS_RE.match(body)
            if a:
                cur_type = None
                last = {"name": a.group(1), "kind": "alias", "domain": a.group(2).strip(), "note": note,
                        "more": "", "fields": [], "line": n}
                types.append(last)
                continue
            if TYPE_RE.match(line):
                cur_type = {"name": line, "kind": "record", "domain": "", "note": "", "more": "", "fields": [], "line": n}
                types.append(cur_type)
                last = None
                continue
            errors.append(f'line {n}: a line with no indent must be a type ("Name") or an alias ("Name = ...")')
            continue
        if indent != 2:
            errors.append(f"line {n}: a field line must have an indent of 2 spaces (a continuation: {CONT_INDENT} or more)")
            continue
        if cur_type is None:
            errors.append(f"line {n}: a field line with no type above it")
            continue
        body, note = split_note(line)
        f = FIELD_RE.match(body)
        if not f:
            errors.append(f'line {n}: a field line must be "  name[, name] [∈|:|⊆|≥ domain]  (note)"')
            continue
        domain = f.group("domain") or ""
        if "," in top_level(domain):
            errors.append(f"line {n}: one line has fields with different domains; write one line for each domain")
        last = {"names": f.group("names"), "op": f.group("op") or "", "domain": domain, "note": note, "more": "", "line": n}
        cur_type["fields"].append(last)
    for item in [*types, *(fl for t in types for fl in t["fields"])]:
        m = item["more"]
        if m.startswith("(") and m.endswith(")") and split_note("  " + m)[1] == m[1:-1].strip():
            item["more"] = m[1:-1].strip()
    seen = {}
    for t in types:
        if t["name"] in seen:
            errors.append(f"line {t['line']}: the type {t['name']} is also on line {seen[t['name']]}")
        seen.setdefault(t["name"], t["line"])
        keys = {}
        for fl in t["fields"]:
            if fl["names"] in keys:
                errors.append(f"line {fl['line']}: the field {t['name']}.{fl['names']} is also on line {keys[fl['names']]}")
            keys.setdefault(fl["names"], fl["line"])
    names = set(seen)
    for t in types:
        t["refs"] = type_refs(t["domain"], names)
        for fl in t["fields"]:
            fl["refs"] = type_refs(fl["domain"], names)
    return types, errors


def mermaid(types):
    """A Mermaid class diagram of the types and the fields that refer to other types."""
    out = ["classDiagram", "  direction LR"]
    for t in types:
        out.append(f"  class {t['name']}")
        if t["kind"] == "alias":
            out.append(f"  <<alias>> {t['name']}")
    for t in types:
        for fl in t["fields"]:
            for r in fl["refs"]:
                d = fl["domain"]
                card = ' "0..*"' if re.search(r"\b(list|set|stacks?) of\b", d) else ' "0..1"' if "{None} ∪" in d else ""
                label = re.sub(r"[^a-z0-9_, ]", "", fl["names"])
                out.append(f"  {t['name']} -->{card} {r} : {label}")
        for r in t["refs"]:
            out.append(f"  {t['name']} ..> {r}")
    return "\n".join(out)


def parse(md):
    lines = md.splitlines()
    sections, rules, problems = [], [], []
    current = None
    seen = {}
    for n, line in enumerate(lines, 1):
        h = HEADING_RE.match(line)
        if h:
            current = {"num": h.group(2), "title": h.group(3).strip(), "level": len(h.group(1)), "note": []}
            sections.append(current)
            continue
        if current is None:
            continue
        m = ROW_RE.match(line)
        if m:
            cells = split_row(line)
            if len(cells) != 3:
                problems.append(f"line {n}: the row of {m.group(1)} has {len(cells)} cells, not 3")
            rid = cells[0]
            if rid in seen:
                problems.append(f"line {n}: {rid} is also on line {seen[rid]}")
            seen[rid] = n
            rules.append(
                {
                    "id": rid,
                    "text": cells[1] if len(cells) > 1 else "",
                    "source": " | ".join(cells[2:]) if len(cells) > 2 else "",
                    "section": current["num"],
                    "line": n,
                }
            )
        elif line.strip() and not line.startswith("|") and current["num"] != "0":
            current["note"].append(line.strip())
    for s in sections:
        s["note"] = " ".join(s["note"])

    model_first = 0
    model = between(lines, "state-model:start", "state-model:end")
    if model is not None:
        model_first = next(i for i, l in enumerate(lines) if l.strip() == "<!-- invariants:state-model:start -->") + 1
    else:  # An earlier version with no markers: the first code block of §0.
        try:
            a = next(i for i, l in enumerate(lines) if l.startswith("## 0."))
            b = next(i for i, l in enumerate(lines) if i > a and l.startswith("```"))
            c = next(i for i, l in enumerate(lines) if i > b and l.startswith("```"))
            model, model_first = lines[b : c + 1], b
        except StopIteration:
            model = []
    if model and model[0].startswith("```"):
        model, model_first = model[1:], model_first + 1
    model = [l for l in model if not l.startswith("```")]
    # An earlier version can break the grammar: then the page shows only the lines that it can read.
    model_types, model_errors = parse_model(model, model_first + 1)

    term_lines = between(lines, "terms:start", "terms:end")
    if term_lines is None:  # An earlier version with no markers.
        term_lines = []
        try:
            a = next(i for i, l in enumerate(lines) if l.strip() == "Terms:")
            for l in lines[a + 1 :]:
                if l.startswith("#"):
                    break
                term_lines.append(l)
        except StopIteration:
            pass
    terms, cur = [], None
    for l in term_lines:
        t = TERM_RE.match(l)
        if t:
            cur = {"name": t.group(1), "text": t.group(2)}
            terms.append(cur)
        elif cur and l.startswith("  ") and l.strip():
            cur["text"] += " " + l.strip()
    return {
        "sections": [{k: s[k] for k in ("num", "title", "level", "note")} for s in sections],
        "rules": rules,
        "model": "\n".join(model).strip("\n"),
        "types": model_types,
        "modelErrors": model_errors,
        "mermaid": mermaid(model_types),
        "terms": terms,
        "problems": problems,
    }


SEPARATOR_RE = re.compile(r"^\|(\s*:?-{3,}:?\s*\|)+\s*$")
HEADER_ROW = ["ID", "Rule", "Source"]


def check(md):
    """The format problems of INVARIANTS.md: each line that the visualizer cannot read correctly.

    The format is in the comment at the start of INVARIANTS.md. An empty list: no problem."""
    errors = []
    lines = md.splitlines()

    # The marker comments: each one time, in the correct order.
    names = ["state-model:start", "state-model:end", "terms:start", "terms:end"]
    where = {}
    for name in names:
        found = [n for n, l in enumerate(lines, 1) if l.strip() == f"<!-- invariants:{name} -->"]
        if len(found) != 1:
            errors.append(f"the marker <!-- invariants:{name} --> must be there one time, not {len(found)} times")
        else:
            where[name] = found[0]
    if len(where) == 4 and not (
        where["state-model:start"] < where["state-model:end"] < where["terms:start"] < where["terms:end"]
    ):
        errors.append("the markers are not in the order state-model:start, state-model:end, terms:start, terms:end")

    if len(where) == 4:
        model = lines[where["state-model:start"] : where["state-model:end"] - 1]
        if len(model) < 3 or model[0].strip() != "```" or model[-1].strip() != "```":
            errors.append(
                f"line {where['state-model:start']}: the state model must be one code block between its markers"
            )
        else:
            types, model_errors = parse_model(model[1:-1], where["state-model:start"] + 2)
            errors += model_errors
            if not types:
                errors.append("the state model has no type")
        terms = 0
        for n in range(where["terms:start"] + 1, where["terms:end"]):
            l = lines[n - 1]
            if TERM_RE.match(l):
                terms += 1
            elif l.strip() and not (l.startswith("  ") and terms):
                errors.append(f'line {n}: a term must be "- **name**: text" (or an indented next line of a term)')
        if terms == 0:
            errors.append("there is no term between the terms markers")

    # The sections and the tables.
    section = None
    seen = {}
    in_code = False
    for n, l in enumerate(lines, 1):
        if l.startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        if re.match(r"^#{2,3} ", l):
            h = HEADING_RE.match(l)
            if h:
                section = h.group(2)
            elif l.strip() != "## Contents":
                errors.append(f'line {n}: a section heading must be "## N. Title" or "### N.M Title"')
            continue
        if not l.startswith("|"):
            continue
        if section is None:
            errors.append(f"line {n}: a table row before the first numbered section")
            continue
        if SEPARATOR_RE.match(l):
            continue
        if l.count("`") % 2:
            errors.append(f"line {n}: a code span is not closed (an odd number of backticks)")
            continue
        cells = split_row(l)
        if cells == HEADER_ROW:
            continue
        if not ROW_RE.match(l):
            errors.append(
                f"line {n}: a table row must be a rule (| ID | Rule | Source |, ID like BRD-3) or the header row"
            )
            continue
        if len(cells) != 3:
            errors.append(f'line {n}: the row of {cells[0]} has {len(cells)} cells, not 3 (write "|" only in a code span)')
            continue
        if not cells[1] or not cells[2]:
            errors.append(f"line {n}: the rule {cells[0]} has no text or no source")
        if not l.rstrip().endswith("|"):
            errors.append(f'line {n}: the row of {cells[0]} must end with "|" (no line break in a row)')
        if cells[0] in seen:
            errors.append(f"line {n}: the ID {cells[0]} is also on line {seen[cells[0]]}")
        seen.setdefault(cells[0], n)

    if not seen:
        errors.append("there are no rules")

    # The references to rules: each ID with the prefix of a rule must be a rule.
    prefixes = {i.split("-")[0] for i in seen}
    for n, l in enumerate(lines, 1):
        for m in ID_RE.finditer(l):
            rid = m.group(1)
            if rid.split("-")[0] in prefixes and rid not in seen:
                errors.append(f"line {n}: {rid} is not a rule")
    return errors


def citations(ids):
    """For each rule ID, the files in tests/ and src/ that give it, with the number of times."""
    out = {i: [] for i in ids}
    files = sorted(
        p for d in ("tests", "src") for p in (ROOT / d).rglob("*") if p.is_file() and p.suffix in (".rs", ".sh", "")
    )
    for p in files:
        try:
            text = p.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        counts = {}
        for m in ID_RE.finditer(text):
            if m.group(1) in out:
                counts[m.group(1)] = counts.get(m.group(1), 0) + 1
        rel = str(p.relative_to(ROOT))
        for rid, c in counts.items():
            out[rid].append([rel, c])
    return out


def versions(history):
    """The earlier versions of INVARIANTS.md: the tags and the last commits that changed it."""
    found = {}
    log = git("log", f"-n{history}", "--format=%H", "--", DOC).split()
    tags = {}
    for tag in git("tag").split():
        tags.setdefault(git("rev-list", "-n1", tag).strip(), []).append(tag)
    for full in [*log, *tags, git("rev-parse", "HEAD").strip()]:
        found[full] = describe(full, tags.get(full, []))
    return found


def describe(full, tags=()):
    short, ts, date, subject = git("log", "-1", "--format=%h%x09%ct%x09%ad%x09%s", "--date=short", full).strip().split("\t", 3)
    label = " · ".join([*tags, f"{short} {date} {subject}"])
    return {"ref": short, "label": label, "time": int(ts)}


def resolve(ref):
    return git("rev-parse", "--verify", f"{ref}^{{commit}}", check=False)


def default_base():
    branch = (git("rev-parse", "--abbrev-ref", "HEAD") or "").strip()
    if branch not in ("main", "HEAD"):
        mb = git("merge-base", "HEAD", "main", check=False)
        if mb:
            return mb.strip(), f"merge base with main (branch {branch})"
    if git("diff", "--quiet", "HEAD", "--", DOC, check=False) is None:
        return resolve("HEAD").strip(), "HEAD (INVARIANTS.md has changes that are not committed)"
    last = git("log", "-n1", "--format=%H", "--", DOC).strip()
    parent = resolve(f"{last}~1")
    if parent:
        return parent.strip(), "the version before the last change of INVARIANTS.md"
    return last, "the last change of INVARIANTS.md"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", help="the git ref to compare with (default: refer to the description)")
    ap.add_argument("--out", default=str(ROOT / "target" / "invariants.html"), help="the HTML file to write")
    ap.add_argument("--history", type=int, default=25, help="the number of earlier commits in the page")
    ap.add_argument("--open", action="store_true", help="open the page in the browser")
    ap.add_argument("--check", action="store_true", help="only examine the format of the file; exit 1 on a problem")
    ap.add_argument("--file", default=str(ROOT / DOC), help="the file to examine with --check")
    args = ap.parse_args()

    if args.check:
        errors = check(Path(args.file).read_text(encoding="utf-8"))
        for e in errors:
            print(f"{args.file}: {e}", file=sys.stderr)
        if errors:
            sys.exit(1)
        print(f"{args.file}: the format is correct")
        return

    current_md = (ROOT / DOC).read_text(encoding="utf-8")
    current = parse(current_md)
    current["problems"] = check(current_md)
    for p in current["problems"]:
        print(f"{DOC}: {p}", file=sys.stderr)

    if args.base:
        full = resolve(args.base)
        if not full:
            sys.exit(f"unknown git ref: {args.base}")
        base_full, why = full.strip(), f"--base {args.base}"
    else:
        base_full, why = default_base()

    vers = versions(args.history)
    if base_full not in vers:
        vers[base_full] = describe(base_full)
    head = resolve("HEAD").strip()

    out_versions = []
    for full, v in sorted(vers.items(), key=lambda kv: kv[1]["time"], reverse=True):
        md = git("show", f"{full}:{DOC}", check=False)
        if md is None:
            continue
        v = dict(v, id=full, doc=parse(md))
        if full == head:
            v["label"] = "HEAD · " + v["label"]
        out_versions.append(v)

    data = {
        "current": current,
        "dirty": current_md != (git("show", f"HEAD:{DOC}", check=False) or ""),
        "versions": out_versions,
        "base": base_full,
        "baseWhy": why,
        "cites": citations([r["id"] for r in current["rules"]]),
        "branch": (git("rev-parse", "--abbrev-ref", "HEAD") or "").strip(),
    }
    html = TEMPLATE.replace("/*DATA*/null", json.dumps(data, ensure_ascii=False).replace("</", "<\\/"))
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(html, encoding="utf-8")
    print(f"wrote {out.resolve()} ({len(current['rules'])} rules, base {why})")
    if args.open:
        webbrowser.open(out.resolve().as_uri())


TEMPLATE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Invariants</title>
<style>
:root {
  --bg: #fbfaf8; --panel: #ffffff; --ink: #1d1d1b; --muted: #6b6a66; --line: #e4e2dc;
  --accent: #2f5fd0; --code-bg: #f1efe9;
  --add: #1f7a3d; --add-bg: #e2f4e7; --chg: #9a5b00; --chg-bg: #fcefd8;
  --del: #b42318; --del-bg: #fde4e1; --mov: #5b3fb5; --mov-bg: #ece7fb;
  --hl: #fff3b0;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #16171a; --panel: #1e2024; --ink: #e8e6e1; --muted: #9a988f; --line: #2f3238;
    --accent: #7aa2ff; --code-bg: #2a2d33;
    --add: #6fd08c; --add-bg: #173323; --chg: #f0b050; --chg-bg: #3a2c12;
    --del: #ff8a7a; --del-bg: #3d1c19; --mov: #b9a5ff; --mov-bg: #2a2342; --hl: #4a4220;
  }
}
:root[data-theme="dark"] {
  --bg: #16171a; --panel: #1e2024; --ink: #e8e6e1; --muted: #9a988f; --line: #2f3238;
  --accent: #7aa2ff; --code-bg: #2a2d33;
  --add: #6fd08c; --add-bg: #173323; --chg: #f0b050; --chg-bg: #3a2c12;
  --del: #ff8a7a; --del-bg: #3d1c19; --mov: #b9a5ff; --mov-bg: #2a2342; --hl: #4a4220;
}
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--ink);
  font: 15px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, sans-serif; }
code { font: 0.88em ui-monospace, SFMono-Regular, Menlo, monospace; background: var(--code-bg);
  padding: 0.05em 0.3em; border-radius: 4px; }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
header { position: sticky; top: 0; z-index: 5; background: var(--bg); border-bottom: 1px solid var(--line);
  padding: 10px 16px; display: flex; flex-wrap: wrap; gap: 8px 14px; align-items: center; }
header h1 { font-size: 16px; margin: 0 6px 0 0; }
header select, header input { font: inherit; font-size: 13px; color: var(--ink); background: var(--panel);
  border: 1px solid var(--line); border-radius: 6px; padding: 4px 8px; }
header input[type=search] { width: 220px; }
header select { max-width: 360px; }
.seg { display: inline-flex; border: 1px solid var(--line); border-radius: 6px; overflow: hidden; }
.seg button { font: inherit; font-size: 13px; border: 0; background: var(--panel); color: var(--ink);
  padding: 4px 10px; cursor: pointer; border-right: 1px solid var(--line); }
.seg button:last-child { border-right: 0; }
.seg button[aria-pressed=true] { background: var(--ink); color: var(--bg); }
.summary { display: flex; gap: 6px; flex-wrap: wrap; font-size: 13px; }
.pill { border-radius: 999px; padding: 1px 9px; font-size: 12px; font-weight: 600; white-space: nowrap; }
.pill.added { color: var(--add); background: var(--add-bg); }
.pill.changed { color: var(--chg); background: var(--chg-bg); }
.pill.removed { color: var(--del); background: var(--del-bg); }
.pill.moved { color: var(--mov); background: var(--mov-bg); }
.pill.same { color: var(--muted); background: var(--code-bg); }
.why { color: var(--muted); font-size: 12px; }
.layout { display: grid; grid-template-columns: 250px minmax(0, 1fr); gap: 0; }
nav { position: sticky; top: 53px; align-self: start; max-height: calc(100vh - 53px); overflow: auto;
  border-right: 1px solid var(--line); padding: 12px 8px 40px; font-size: 13px; }
nav a { display: flex; justify-content: space-between; gap: 6px; padding: 3px 8px; border-radius: 6px; color: var(--ink); }
nav a:hover { background: var(--code-bg); text-decoration: none; }
nav a.sub { padding-left: 22px; }
nav .n { color: var(--muted); font-variant-numeric: tabular-nums; }
nav .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; background: var(--chg); margin-right: 4px; }
main { padding: 14px 20px 80px; max-width: 1100px; }
.problems { border: 1px solid var(--del); background: var(--del-bg); color: var(--del); border-radius: 8px;
  padding: 8px 12px; margin-bottom: 12px; font-size: 13px; }
section > h2 { font-size: 18px; margin: 26px 0 4px; }
section > h3 { font-size: 16px; margin: 20px 0 4px; }
.note { color: var(--muted); font-size: 13px; margin: 0 0 8px; }
.rule { background: var(--panel); border: 1px solid var(--line); border-left: 4px solid var(--line);
  border-radius: 8px; padding: 10px 14px; margin: 8px 0; scroll-margin-top: 70px; }
.rule.added { border-left-color: var(--add); }
.rule.changed { border-left-color: var(--chg); }
.rule.removed { border-left-color: var(--del); opacity: 0.85; }
.rule.moved { border-left-color: var(--mov); }
.rule.flash { box-shadow: 0 0 0 3px var(--hl); }
.rule .top { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.rule .rid { font: 600 14px ui-monospace, SFMono-Regular, Menlo, monospace; color: var(--ink); }
.rule .tools { margin-left: auto; display: flex; gap: 8px; font-size: 12px; }
.rule .tools button { font: inherit; background: none; border: 1px solid var(--line); color: var(--muted);
  border-radius: 5px; padding: 1px 7px; cursor: pointer; }
.rule .text { margin: 6px 0 4px; }
.rule.removed .text { text-decoration: line-through; color: var(--muted); }
.meta { font-size: 12.5px; color: var(--muted); display: grid; grid-template-columns: max-content 1fr; gap: 2px 10px; }
.meta b { font-weight: 600; }
.refs a { margin-right: 6px; font-family: ui-monospace, Menlo, monospace; font-size: 12px; }
.cites span { margin-right: 10px; white-space: nowrap; }
.cites .none { color: var(--del); }
a.ref { font-family: ui-monospace, Menlo, monospace; font-size: 0.92em; }
a.ref.gone { color: var(--del); text-decoration: line-through; }
ins { background: var(--add-bg); color: var(--add); text-decoration: none; border-radius: 3px; }
del { background: var(--del-bg); color: var(--del); border-radius: 3px; }
.diffsrc { font-size: 12.5px; color: var(--muted); }
details.block { background: var(--panel); border: 1px solid var(--line); border-radius: 8px; margin: 10px 0; }
details.block > summary { cursor: pointer; padding: 8px 14px; font-weight: 600; }
details.block pre { margin: 0; padding: 8px 14px 14px; overflow-x: auto; font: 12.5px/1.45 ui-monospace, Menlo, monospace; }
pre .l-add { background: var(--add-bg); color: var(--add); display: block; }
pre .l-del { background: var(--del-bg); color: var(--del); display: block; }
pre .l-add, pre .l-del { min-height: 1.45em; }
pre .l-fold { display: block; color: var(--muted); font-style: italic; padding: 2px 0; cursor: pointer; }
.block-old, .block-new { display: block; margin: 2px 0; }
.types { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 480px), 1fr)); gap: 10px; padding: 4px 14px 14px; }
.type { border: 1px solid var(--line); border-left: 4px solid var(--line); border-radius: 8px; overflow: hidden; scroll-margin-top: 70px; background: var(--panel); }
.type.added { border-left-color: var(--add); } .type.changed { border-left-color: var(--chg); } .type.removed { border-left-color: var(--del); }
.type.flash { box-shadow: 0 0 0 3px var(--hl); }
.type > h4 { margin: 0; padding: 6px 12px; font: 600 14px ui-monospace, Menlo, monospace; display: flex; gap: 8px; align-items: center;
  background: var(--code-bg); }
.type > h4 .why { font-family: inherit; font-weight: 400; margin-left: auto; }
.type .def { padding: 6px 12px; font: 12.5px/1.5 ui-monospace, Menlo, monospace; }
.type .def .note { margin: 4px 0 0; }
.fields { width: 100%; border-collapse: collapse; font-size: 13px; }
.fields td { border-top: 1px solid var(--line); padding: 4px 8px; vertical-align: top; }
.fields td.nm { font: 600 12.5px ui-monospace, Menlo, monospace; width: 34%; overflow-wrap: anywhere; }
.fields td.dom { font: 12.5px ui-monospace, Menlo, monospace; }
.fields .nt { display: block; color: var(--muted); font: 12.5px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, sans-serif; margin-top: 1px; }
.fields tr.added td { background: var(--add-bg); } .fields tr.changed td { background: var(--chg-bg); }
.fields tr.removed td { background: var(--del-bg); text-decoration: line-through; color: var(--del); }
.fields tr.fold td { color: var(--muted); font-style: italic; font-size: 12px; }
.fields .more { display: block; margin-top: 2px; }
a.tref { font-weight: 600; }
.diagram { padding: 4px 14px 10px; overflow-x: auto; }
.diagram svg { max-width: 100%; height: auto; }
.diagram .why { display: block; }
details.src > summary { cursor: pointer; padding: 4px 14px 8px; color: var(--muted); font-size: 13px; }
.term { padding: 6px 14px; border-top: 1px solid var(--line); font-size: 14px; }
.term.added { background: var(--add-bg); } .term.removed { background: var(--del-bg); }
.term.changed { background: var(--chg-bg); }
.empty { color: var(--muted); padding: 30px 0; text-align: center; }
.hidden { display: none !important; }
#tip { position: fixed; z-index: 20; max-width: 520px; background: var(--panel); color: var(--ink);
  border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; font-size: 13px;
  box-shadow: 0 6px 24px rgba(0,0,0,.18); pointer-events: none; }
kbd { font: 11px ui-monospace, Menlo, monospace; border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; }
@media (max-width: 820px) {
  .layout { grid-template-columns: 1fr; }
  nav { display: none; }
  main { padding: 12px 16px 60px; }
  header { position: static; }
  header input[type=search] { width: 100%; }
}
</style>
</head>
<body>
<header>
  <h1>Invariants</h1>
  <label class="why">Base <select id="base"></select></label>
  <div class="seg" id="view">
    <button data-v="changes" aria-pressed="false">Changes</button>
    <button data-v="all" aria-pressed="true">All rules</button>
  </div>
  <input type="search" id="q" placeholder="Search: text, ID, R§9.3…  ( / )">
  <div class="summary" id="summary"></div>
  <span class="why" id="why"></span>
  <span class="why"><kbd>n</kbd>/<kbd>p</kbd> next or previous change</span>
</header>
<div class="layout">
  <nav id="nav"></nav>
  <main id="main"></main>
</div>
<div id="tip" class="hidden"></div>
<script>
const DATA = /*DATA*/null;
const $ = (s, el = document) => el.querySelector(s);
const esc = s => s.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const ID_RE = /\b([A-Z]{2,4}-\d+)\b/g;
const cur = DATA.current;
const curById = new Map(cur.rules.map(r => [r.id, r]));
const secTitle = new Map(cur.sections.map(s => [s.num, s.title]));
const PREFIXES = new Set(cur.rules.map(r => r.id.split("-")[0]));

let state = { base: DATA.base, view: "all", q: "" };
try {
  const h = new URLSearchParams(location.hash.slice(1));
  if (h.get("base") && DATA.versions.some(v => v.id.startsWith(h.get("base")))) state.base = DATA.versions.find(v => v.id.startsWith(h.get("base"))).id;
  if (h.get("view")) state.view = h.get("view");
  if (h.get("q")) state.q = h.get("q");
} catch (e) {}
if (DATA.versions.length && state.view === "all" && !location.hash) state.view = "changes";

/* Inline Markdown: code spans, bold, italic, links, rule IDs. */
function md(text, known) {
  return text.split(/(`[^`]*`)/).map(part => {
    if (part.startsWith("`") && part.endsWith("`") && part.length > 1) return "<code>" + esc(part.slice(1, -1)) + "</code>";
    let h = esc(part)
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/(^|[^*])\*([^*\s][^*]*?)\*/g, "$1<em>$2</em>")
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1");
    return h.replace(ID_RE, id => PREFIXES.has(id.split("-")[0]) ? refLink(id, known) : id);
  }).join("");
}
function refLink(id, known) {
  const ok = (known || curById).has(id);
  return `<a class="ref${ok ? "" : " gone"}" href="#${id}" data-ref="${id}">${id}</a>`;
}

/* Word diff (LCS on words and spaces). */
function tokens(s) { return s.match(/\s+|[^\s]+/g) || []; }
function wordDiff(a, b) {
  const x = tokens(a), y = tokens(b), n = x.length, m = y.length;
  const dp = Array.from({ length: n + 1 }, () => new Uint16Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--)
    dp[i][j] = x[i] === y[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out = []; let i = 0, j = 0;
  const push = (t, s) => { if (out.length && out[out.length - 1][0] === t) out[out.length - 1][1] += s; else out.push([t, s]); };
  while (i < n && j < m) {
    if (x[i] === y[j]) { push("=", x[i]); i++; j++; }
    else if (dp[i + 1][j] >= dp[i][j + 1]) push("-", x[i++]);
    else push("+", y[j++]);
  }
  while (i < n) push("-", x[i++]);
  while (j < m) push("+", y[j++]);
  // Join a short equal run between two changes into the change (less noise).
  const ops = [];
  for (let k = 0; k < out.length; k++) {
    const [t, s] = out[k];
    const short = t === "=" && k > 0 && k < out.length - 1 && s.trim().split(/\s+/).filter(w => w.length > 3).length === 0;
    if (short) { ops.push(["-", s], ["+", s]); continue; }
    ops.push([t, s]);
  }
  let del = "", ins = "", html = "", same = 0;
  const flush = () => { if (del) html += `<del>${esc(del)}</del>`; if (ins) html += `<ins>${esc(ins)}</ins>`; del = ins = ""; };
  for (const [t, s] of ops) {
    if (t === "=") { flush(); html += esc(s); same += s.length; }
    else if (t === "-") del += s; else ins += s;
  }
  flush();
  // When most of the text changed, show the earlier text and the new text as two blocks.
  if (Math.max(a.length, b.length) > 120 && same < 0.4 * Math.max(a.length, b.length))
    return `<span class="block-old"><del>${esc(a)}</del></span><span class="block-new"><ins>${esc(b)}</ins></span>`;
  return html;
}
function lineDiff(a, b, fold) {
  const x = a.split("\n"), y = b.split("\n"), n = x.length, m = y.length;
  const dp = Array.from({ length: n + 1 }, () => new Uint16Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--)
    dp[i][j] = x[i] === y[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const ops = []; let i = 0, j = 0;
  while (i < n && j < m) {
    if (x[i] === y[j]) { ops.push(["=", y[j]]); i++; j++; }
    else if (dp[i + 1][j] >= dp[i][j + 1]) ops.push(["-", x[i++]]);
    else ops.push(["+", y[j++]]);
  }
  while (i < n) ops.push(["-", x[i++]]);
  while (j < m) ops.push(["+", y[j++]]);
  const changed = ops.some(o => o[0] !== "=");
  // In the Changes view, a run of more than 7 lines with no change shows only 3 lines at each end.
  const keep = ops.map(() => true);
  if (fold && changed) {
    for (let k = 0; k < ops.length; ) {
      if (ops[k][0] !== "=") { k++; continue; }
      let e = k; while (e < ops.length && ops[e][0] === "=") e++;
      const lo = k === 0 ? k : k + 3, hi = e === ops.length ? e : e - 3;
      if (hi - lo > 1) for (let t = lo; t < hi; t++) keep[t] = false;
      k = e;
    }
  }
  const out = [];
  for (let k = 0; k < ops.length; k++) {
    if (!keep[k]) {
      let e = k; while (e < ops.length && !keep[e]) e++;
      const hidden = ops.slice(k, e).map(o => esc(o[1]) + "\n").join("");
      out.push(`<span class="l-fold" data-fold>⋯ ${e - k} lines with no change (click to show)</span><span class="hidden">${hidden}</span>`); k = e - 1; continue;
    }
    const [t, l] = ops[k];
    out.push(t === "=" ? esc(l) + "\n" : `<span class="${t === "+" ? "l-add" : "l-del"}">${esc(l)}</span>`);
  }
  return { html: out.join(""), changed };
}

/* The rules that refer to each rule (in the current document). */
const refBy = new Map();
for (const r of cur.rules) for (const m of (r.text + " " + r.source).matchAll(ID_RE))
  if (m[1] !== r.id && PREFIXES.has(m[1].split("-")[0])) { if (!refBy.has(m[1])) refBy.set(m[1], new Set()); refBy.get(m[1]).add(r.id); }

function compare() {
  const v = DATA.versions.find(v => v.id === state.base);
  const base = v ? v.doc : { rules: [], sections: [], model: "", terms: [] };
  const old = new Map(base.rules.map(r => [r.id, r]));
  const status = new Map();
  for (const r of cur.rules) {
    const o = old.get(r.id);
    if (!o) status.set(r.id, "added");
    else if (o.text !== r.text || o.source !== r.source) status.set(r.id, "changed");
    else if (o.section !== r.section) status.set(r.id, "moved");
    else status.set(r.id, "same");
  }
  const removed = base.rules.filter(r => !curById.has(r.id));
  return { v, base, old, status, removed };
}

function ruleCard(r, st, o, removed) {
  const showDiff = st === "changed";
  const text = removed ? md(r.text, new Map()) : md(r.text);
  const diffText = showDiff ? wordDiff(o.text, r.text) : "";
  const srcDiff = showDiff && o.source !== r.source ? wordDiff(o.source, r.source) : "";
  const by = [...(refBy.get(r.id) || [])].sort(byId);
  const cites = (DATA.cites[r.id] || []).slice().sort((a, b) => b[1] - a[1]);
  const testCites = cites.filter(c => c[0].startsWith("tests/"));
  const srcCites = cites.filter(c => c[0].startsWith("src/"));
  const label = st === "same" ? "" : `<span class="pill ${st}">${st}${st === "moved" ? " from §" + esc(o.section) : ""}</span>`;
  const citeList = l => l.map(c => `<span title="${c[1]} times">${esc(c[0].replace(/^(tests|src)\//, ""))} <span class="n">×${c[1]}</span></span>`).join("");
  return `<article class="rule ${st}" id="${removed ? "removed-" : ""}${r.id}" data-id="${r.id}" data-st="${st}"
      data-search="${esc((r.id + " " + r.text + " " + r.source).toLowerCase())}">
    <div class="top"><a class="rid" href="#${r.id}">${r.id}</a>${label}
      <span class="tools">${showDiff ? `<button data-toggle>Show current text</button>` : ""}
      ${removed ? "" : `<span class="why">line ${r.line}</span>`}</span></div>
    <div class="text" data-diff="${showDiff ? 1 : 0}">${showDiff ? diffText : text}</div>
    ${showDiff ? `<template>${text}</template><template>${diffText}</template>` : ""}
    <div class="meta">
      <b>Source</b><span>${srcDiff ? `<span class="diffsrc">${srcDiff}</span>` : md(r.source, removed ? new Map() : null)}</span>
      ${removed ? "" : `
      <b>Used by</b><span class="refs">${by.length ? by.map(i => refLink(i)).join("") : "—"}</span>
      <b>Tests</b><span class="cites">${testCites.length ? citeList(testCites) : srcCites.length ? "— (only in the code)" : `<span class="none">no file gives this ID</span>`}</span>
      ${srcCites.length ? `<b>Code</b><span class="cites">${citeList(srcCites)}</span>` : ""}`}
    </div>
  </article>`;
}
function byId(a, b) {
  const [pa, na] = a.split("-"), [pb, nb] = b.split("-");
  return pa === pb ? +na - +nb : pa < pb ? -1 : 1;
}


/* The state model: one card for each type (the parser of scripts/invariants.py reads the types). */
const TYPE_NAMES = new Set(cur.types.map(t => t.name));
function domainHtml(domain) {
  // Links only the type names out of braces and parentheses (as type_refs in the script).
  let out = "", depth = 0, word = "";
  const flushWord = () => {
    if (!word) return;
    out += depth === 0 && TYPE_NAMES.has(word) ? `<a class="tref" href="#type-${word}" data-type="${word}">${word}</a>` : esc(word);
    word = "";
  };
  for (const ch of domain) {
    if (/[A-Za-z0-9_]/.test(ch)) { word += ch; continue; }
    flushWord();
    if ("{(".includes(ch)) depth++;
    if ("})".includes(ch) && depth) depth--;
    out += esc(ch);
  }
  flushWord();
  return out;
}
const opDomain = f => (f.op ? f.op + " " : "") + f.domain;
const noteText = f => [f.note, f.more].filter(Boolean).join(" · ");
function noteHtml(f) {
  return (f.note ? md(f.note) : "") + (f.more ? `<span class="more">${md(f.more)}</span>` : "");
}
function modelHtml(base, onlyChanges) {
  const oldTypes = new Map((base.types || []).map(t => [t.name, t]));
  let changes = 0;
  const cards = [];
  const card = (t, st, body, line) => `<div class="type ${st}" id="type-${esc(t.name)}">
      <h4>${esc(t.name)}${t.kind === "alias" ? ' <span class="pill same">alias</span>' : ""}${st !== "same" ? ` <span class="pill ${st}">${st}</span>` : ""}
      ${line ? `<span class="why">line ${line}</span>` : ""}</h4>${body}</div>`;
  for (const t of cur.types) {
    const o = oldTypes.get(t.name);
    if (t.kind === "alias") {
      const st = !o ? "added" : (o.domain !== t.domain || noteText(o) !== noteText(t)) ? "changed" : "same";
      if (st !== "same") changes++;
      if (onlyChanges && st === "same") continue;
      const def = st === "changed" ? wordDiff(o.domain, t.domain) : domainHtml(t.domain);
      const note = st === "changed" && noteText(o) !== noteText(t) ? wordDiff(noteText(o), noteText(t)) : noteHtml(t);
      cards.push(card(t, st, `<div class="def" data-search="${esc((t.name + " " + t.domain + " " + noteText(t)).toLowerCase())}">= ${def}${note ? `<div class="note">${note}</div>` : ""}</div>`, t.line));
      continue;
    }
    const oldFields = new Map(((o && o.fields) || []).map(f => [f.names, f]));
    const curNames = new Set(t.fields.map(f => f.names));
    const rows = [];
    let same = 0, typeChanges = 0;
    for (const f of t.fields) {
      const of = oldFields.get(f.names);
      const st = !o || !of ? "added" : (opDomain(of) !== opDomain(f) || noteText(of) !== noteText(f)) ? "changed" : "same";
      if (st !== "same") typeChanges++;
      if (onlyChanges && st === "same") { same++; continue; }
      const dom = st === "changed" && opDomain(of) !== opDomain(f) ? wordDiff(opDomain(of), opDomain(f)) : (f.op ? esc(f.op) + " " : "") + domainHtml(f.domain);
      const note = st === "changed" && noteText(of) !== noteText(f) ? wordDiff(noteText(of), noteText(f)) : noteHtml(f);
      rows.push(`<tr class="${st}" data-search="${esc((t.name + " " + f.names + " " + opDomain(f) + " " + noteText(f)).toLowerCase())}">
        <td class="nm">${esc(f.names)}</td><td class="dom">${dom}${note ? `<span class="nt">${note}</span>` : ""}</td></tr>`);
    }
    for (const of of ((o && o.fields) || []).filter(f => !curNames.has(f.names))) {
      typeChanges++;
      rows.push(`<tr class="removed" data-search="${esc((t.name + " " + of.names + " " + opDomain(of)).toLowerCase())}">
        <td class="nm">${esc(of.names)}</td><td class="dom">${esc(opDomain(of))}${noteText(of) ? `<span class="nt">${esc(noteText(of))}</span>` : ""}</td></tr>`);
    }
    if (same) rows.push(`<tr class="fold"><td colspan="2">${same} fields with no change</td></tr>`);
    const st = !o ? "added" : typeChanges ? "changed" : "same";
    if (st !== "same") changes++;
    if (onlyChanges && st === "same") continue;
    cards.push(card(t, st, `<table class="fields">${rows.join("")}</table>`, t.line));
  }
  for (const o of (base.types || []).filter(t => !TYPE_NAMES.has(t.name))) {
    changes++;
    cards.push(card(o, "removed", `<div class="def"><del>${esc(o.kind === "alias" ? "= " + o.domain : o.fields.map(f => f.names).join(", "))}</del></div>`));
  }
  if (!cards.length) cards.push(`<p class="why">No type changed.</p>`);
  return { html: cards.join(""), changes };
}

/* The diagram of the types (Mermaid, from cdn.jsdelivr.net). The script makes the diagram text. */
let mermaidLoad = null;
function drawDiagram() {
  const box = $("#diagram");
  if (!box || !cur.mermaid) return;
  const dark = document.documentElement.dataset.theme === "dark" ||
    (document.documentElement.dataset.theme !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
  mermaidLoad = mermaidLoad || new Promise((ok, fail) => {
    const sc = document.createElement("script");
    sc.src = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";
    sc.onload = () => ok(window.mermaid); sc.onerror = fail;
    document.head.appendChild(sc);
  });
  mermaidLoad.then(async m => {
    m.initialize({ startOnLoad: false, theme: dark ? "dark" : "neutral", securityLevel: "strict" });
    const { svg } = await m.render("types-diagram", cur.mermaid);
    box.innerHTML = svg;
    // A click on a type in the diagram goes to its card.
    for (const g of box.querySelectorAll("g.node, g.classGroup")) {
      const name = [...TYPE_NAMES].find(n => g.id && new RegExp("(^|-)" + n + "(-|$)").test(g.id));
      if (name) { g.style.cursor = "pointer"; g.addEventListener("click", () => jumpType(name)); }
    }
  }).catch(() => { box.innerHTML = '<span class="why">The diagram needs the network (Mermaid from cdn.jsdelivr.net).</span>'; });
}
function jumpType(name) {
  const el = document.getElementById("type-" + name);
  if (!el) return;
  el.scrollIntoView({ block: "start", behavior: "smooth" });
  el.classList.add("flash"); setTimeout(() => el.classList.remove("flash"), 1200);
}

function render() {
  const { v, base, old, status, removed } = compare();
  const counts = { added: 0, changed: 0, moved: 0, removed: removed.length, same: 0 };
  for (const s of status.values()) counts[s]++;
  const onlyChanges = state.view === "changes";

  $("#summary").innerHTML = ["added", "changed", "moved", "removed"].filter(k => counts[k])
    .map(k => `<span class="pill ${k}">${counts[k]} ${k}</span>`).join("")
    + (counts.added + counts.changed + counts.moved + counts.removed ? "" : `<span class="pill same">no rule changed</span>`)
    + `<span class="pill same">${cur.rules.length} rules</span>`;
  $("#why").textContent = state.base === DATA.base ? "default: " + DATA.baseWhy + (DATA.dirty ? " · working tree has changes" : "") : (DATA.dirty ? "working tree has changes" : "");

  // The state model and the terms.
  const model = lineDiff(base.model || "", cur.model, true);
  const oldTerms = new Map((base.terms || []).map(t => [t.name, t.text]));
  const curTerms = new Set(cur.terms.map(t => t.name));
  const termRows = cur.terms.map(t => {
    const o = oldTerms.get(t.name);
    const st = o === undefined ? "added" : o !== t.text ? "changed" : "same";
    return { st, html: `<div class="term ${st}" data-search="${esc((t.name + " " + t.text).toLowerCase())}"><strong>${esc(t.name)}</strong>: ${st === "changed" ? wordDiff(o, t.text) : md(t.text)}</div>` };
  }).concat((base.terms || []).filter(t => !curTerms.has(t.name)).map(t =>
    ({ st: "removed", html: `<div class="term removed" data-search="${esc((t.name + " " + t.text).toLowerCase())}"><del><strong>${esc(t.name)}</strong>: ${esc(t.text)}</del></div>` })));
  const termChanges = termRows.filter(t => t.st !== "same").length;

  const types = modelHtml(base, onlyChanges);

  let html = cur.problems.length ? `<div class="problems"><b>INVARIANTS.md has format problems:</b><br>${cur.problems.map(esc).join("<br>")}</div>` : "";
  html += `<section data-sec="0"><h2 id="sec-0">0. State model</h2>
    <details class="block" id="model" open><summary>State model (${cur.types.length} types) ${types.changes ? `<span class="pill changed">${types.changes} changed</span>` : ""}</summary>
      ${onlyChanges ? "" : `<div class="diagram" id="diagram"></div>`}
      <div class="types">${types.html}</div>
      <details class="src" ${model.changed && !types.changes ? "open" : ""}><summary>Source text of the state model${model.changed ? " (changed)" : ""}</summary><pre>${model.html}</pre></details></details>
    <details class="block" ${termChanges ? "open" : ""}><summary>Terms (${cur.terms.length}) ${termChanges ? `<span class="pill changed">${termChanges} changed</span>` : ""}</summary>
      ${termRows.filter(t => !onlyChanges || t.st !== "same").map(t => t.html).join("")}</details></section>`;

  const nav = [`<a href="#sec-0"><span>${model.changed || termChanges ? '<span class="dot"></span>' : ""}0. State model</span><span class="n">${cur.types.length}</span></a>`];
  for (const s of cur.sections) {
    if (s.num === "0") continue;
    const rules = cur.rules.filter(r => r.section === s.num);
    const gone = removed.filter(r => (r.section === s.num));
    const changedHere = rules.filter(r => status.get(r.id) !== "same").length + gone.length;
    const shown = rules.filter(r => !onlyChanges || status.get(r.id) !== "same");
    const goneShown = gone;
    nav.push(`<a href="#sec-${s.num}" class="${s.level === 3 ? "sub" : ""}"><span>${changedHere ? '<span class="dot"></span>' : ""}${esc(s.num)}${s.level === 3 ? "" : "."} ${esc(s.title)}</span><span class="n">${rules.length}</span></a>`);
    if (onlyChanges && !shown.length && !goneShown.length) continue;
    const H = s.level === 3 ? "h3" : "h2";
    html += `<section data-sec="${esc(s.num)}"><${H} id="sec-${esc(s.num)}">${esc(s.num)}${s.level === 3 ? "" : "."} ${esc(s.title)}</${H}>
      ${s.note && !onlyChanges ? `<p class="note">${md(s.note)}</p>` : ""}
      ${shown.map(r => ruleCard(r, status.get(r.id), old.get(r.id), false)).join("")}
      ${goneShown.map(r => ruleCard(r, "removed", r, true)).join("")}</section>`;
  }
  // Removed rules of a section that is not in the current document.
  const orphan = removed.filter(r => !cur.sections.some(s => s.num === r.section));
  if (orphan.length) html += `<section><h2>Removed sections</h2>${orphan.map(r => ruleCard(r, "removed", r, true)).join("")}</section>`;
  if (onlyChanges && !(counts.added + counts.changed + counts.moved + counts.removed) && !model.changed && !termChanges)
    html += `<p class="empty">No rule changed against ${esc(v ? v.label : state.base)}. Select a different base, or <a href="#" data-view="all">show all rules</a>.</p>`;
  $("#main").innerHTML = html;
  $("#nav").innerHTML = nav.join("");
  for (const b of document.querySelectorAll("#view button")) b.setAttribute("aria-pressed", b.dataset.v === state.view);
  applySearch();
  saveHash();
  if (!onlyChanges) drawDiagram();
}

function applySearch() {
  const q = state.q.trim().toLowerCase();
  const words = q.split(/\s+/).filter(Boolean);
  let n = 0;
  for (const el of document.querySelectorAll("#main [data-search]")) {
    const hit = words.every(w => el.dataset.search.includes(w));
    el.classList.toggle("hidden", !hit);
    if (hit && el.classList.contains("rule")) n++;
  }
  $("#diagram")?.classList.toggle("hidden", words.length > 0);
  $("#model .src")?.classList.toggle("hidden", words.length > 0);
  for (const c of document.querySelectorAll("#main .type"))
    c.classList.toggle("hidden", words.length > 0 && !c.querySelector("tr[data-search]:not(.hidden), .def[data-search]:not(.hidden)"));
  for (const s of document.querySelectorAll("#main section[data-sec]"))
    if (s.dataset.sec !== "0") s.classList.toggle("hidden", words.length > 0 && !s.querySelector(".rule:not(.hidden)"));
}
function saveHash() {
  const h = new URLSearchParams();
  if (state.base !== DATA.base) h.set("base", state.base.slice(0, 10));
  h.set("view", state.view);
  if (state.q) h.set("q", state.q);
  history.replaceState(null, "", "#" + h.toString());
}

// Controls.
const sel = $("#base");
sel.innerHTML = DATA.versions.map(v => `<option value="${v.id}">${esc(v.label)}${v.id === DATA.base ? " (default)" : ""}</option>`).join("");
sel.value = state.base;
sel.onchange = () => { state.base = sel.value; render(); };
$("#q").value = state.q;
$("#q").oninput = e => { state.q = e.target.value; applySearch(); saveHash(); };
$("#view").onclick = e => { if (e.target.dataset.v) { state.view = e.target.dataset.v; render(); } };

function jump(id) {
  let el = document.getElementById(id);
  if (!el && state.view === "changes") { state.view = "all"; render(); el = document.getElementById(id); }
  if (!el) return;
  if (el.classList.contains("hidden")) { state.q = ""; $("#q").value = ""; applySearch(); }
  el.scrollIntoView({ block: "start", behavior: "smooth" });
  el.classList.add("flash"); setTimeout(() => el.classList.remove("flash"), 1200);
}
document.addEventListener("click", e => {
  const tr = e.target.closest("a[data-type]");
  if (tr) { e.preventDefault(); jumpType(tr.dataset.type); return; }
  const a = e.target.closest("a[data-ref], a.rid");
  if (a) { e.preventDefault(); jump(a.dataset.ref || a.getAttribute("href").slice(1)); return; }
  const v = e.target.closest("[data-view]");
  if (v) { e.preventDefault(); state.view = v.dataset.view; render(); return; }
  const f = e.target.closest("[data-fold]");
  if (f) { f.nextElementSibling.classList.remove("hidden"); f.remove(); return; }
  const t = e.target.closest("[data-toggle]");
  if (t) {
    const card = t.closest(".rule"), box = $(".text", card), [cur, diff] = card.querySelectorAll("template");
    const toDiff = box.dataset.diff !== "1";
    box.innerHTML = (toDiff ? diff : cur).innerHTML; box.dataset.diff = toDiff ? 1 : 0;
    t.textContent = toDiff ? "Show current text" : "Show changes";
  }
});
// A tooltip with the text of a rule that a link refers to.
const tip = $("#tip");
document.addEventListener("mouseover", e => {
  const a = e.target.closest("a.ref, .refs a");
  if (!a) return;
  const r = curById.get(a.dataset.ref);
  tip.innerHTML = r ? `<b>${r.id}</b> · §${esc(r.section)} ${esc(secTitle.get(r.section) || "")}<br>${md(r.text)}` : `<b>${esc(a.dataset.ref)}</b>: not in the current document`;
  tip.classList.remove("hidden");
  const b = a.getBoundingClientRect();
  tip.style.left = Math.min(b.left, innerWidth - tip.offsetWidth - 12) + "px";
  tip.style.top = (b.bottom + 6 + tip.offsetHeight > innerHeight ? b.top - tip.offsetHeight - 6 : b.bottom + 6) + "px";
});
document.addEventListener("mouseout", e => { if (e.target.closest("a.ref, .refs a")) tip.classList.add("hidden"); });
document.addEventListener("keydown", e => {
  if (e.target.matches("input, select")) { if (e.key === "Escape") e.target.blur(); return; }
  if (e.key === "/") { e.preventDefault(); $("#q").focus(); }
  if (e.key === "n" || e.key === "p") {
    const list = [...document.querySelectorAll(".rule:not(.same):not(.hidden)")];
    if (!list.length) return;
    const y = 80;
    const idx = list.findIndex(el => el.getBoundingClientRect().top > y + 2);
    let target;
    if (e.key === "n") target = idx === -1 ? list[0] : list[idx];
    else { const before = list.filter(el => el.getBoundingClientRect().top < y - 2); target = before.length ? before[before.length - 1] : list[list.length - 1]; }
    target.scrollIntoView({ block: "start", behavior: "smooth" });
    target.classList.add("flash"); setTimeout(() => target.classList.remove("flash"), 900);
  }
});
render();
</script>
</body>
</html>
"""

if __name__ == "__main__":
    main()
