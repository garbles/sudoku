#!/usr/bin/env python3
"""Make assets/licenses.json: the third-party crates in the tablet program and their licenses.

The script reads `cargo metadata` for the tablet target (aarch64-unknown-linux-musl). It starts at
the crate rm-sudoku and follows only the normal dependencies. It does not follow the build
dependencies, the dev dependencies and the proc-macro crates: their code does not go into the
program. For each crate, it keeps the name, the version, the SPDX license expression and the text of
each license file in the package (LICENSE*, LICENCE*, COPYING*, NOTICE*, UNLICENSE*). Two crates
with the same text share one copy of it.

Usage:
    python3 scripts/licenses.py           # write assets/licenses.json
    python3 scripts/licenses.py --check   # fail if assets/licenses.json is not up to date
"""

import json
import os
import re
import subprocess
import sys

TARGET = "aarch64-unknown-linux-musl"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "licenses.json")
PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")


def metadata():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)


def is_proc_macro(package):
    return any("proc-macro" in t["kind"] for t in package["targets"])


def shipped(meta):
    """The ids of the third-party packages that the tablet program contains."""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        pid = stack.pop()
        if pid in seen or is_proc_macro(packages[pid]):
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    seen.discard(root)
    return [packages[i] for i in seen]


def normalize(text):
    text = text.replace("\r\n", "\n").replace("\r", "\n").replace("\t", "    ")
    lines = [line.rstrip() for line in text.split("\n")]
    text = "\n".join(lines).strip("\n")
    return re.sub(r"\n{3,}", "\n\n", text)


def license_files(package):
    folder = os.path.dirname(package["manifest_path"])
    names = []
    if package.get("license_file"):
        names.append(package["license_file"])
    for name in sorted(os.listdir(folder)):
        if name.upper().startswith(PREFIXES) and os.path.isfile(os.path.join(folder, name)) and name not in names:
            names.append(name)
    files = []
    for name in names:
        with open(os.path.join(folder, name), encoding="utf-8", errors="replace") as f:
            files.append((os.path.basename(name), normalize(f.read())))
    return files


def sqlite_version(meta):
    """The version of the SQLite source in libsqlite3-sys (the feature `bundled`)."""
    for p in meta["packages"]:
        if p["name"] == "libsqlite3-sys":
            header = os.path.join(os.path.dirname(p["manifest_path"]), "sqlite3", "sqlite3.h")
            with open(header, encoding="utf-8") as f:
                m = re.search(r'#define SQLITE_VERSION\s+"([^"]+)"', f.read())
            return m.group(1)
    raise SystemExit("libsqlite3-sys is not in the dependencies")


SQLITE_NOTICE = """The author disclaims copyright to this source code. In place of a legal notice, here is a blessing:

    May you do good and not evil.
    May you find forgiveness for yourself and forgive others.
    May you share freely, never taking more than you give.

SQLite is in the public domain (https://sqlite.org/copyright.html). The crate libsqlite3-sys includes its source code, and the program contains it."""


UJI_NOTICE = """UJI Pen Characters (Version 2), https://archive.ics.uci.edu/dataset/404/uji+pen+characters+version+2

David Llorens, Federico Prat, Andrés Marzal, Juan Miguel Vilar, María José Castro, Juan Carlos Amengual, Sergio Barrachina, Antonio Castellanos, Salvador España, Jon Ander Gómez, Jorge González, Alfons Juan, Vicente Palazón, María José Ramos, Rosa Sanchis, Alberto Sanchis, Enrique Vidal. "The UJIpenchars Database: a Pen-Based Database of Isolated Handwritten Characters", LREC 2008.

License: Creative Commons Attribution 4.0 International (CC BY 4.0), https://creativecommons.org/licenses/by/4.0/

Changes: the app uses the data as examples of handwriting for the recognizer. The characters with the shape of a 1 are removed. Each example is resampled to approximately 40 points and scaled to a box of 0 to 255. The digits 1 to 9 keep their class, and all other characters have the class "not 1-9"."""


def version_key(version):
    return [(0, int(x), "") if x.isdigit() else (1, 0, x) for x in re.split(r"[.+-]", version)]


def build():
    meta = metadata()
    texts, text_ids, crates = [], {}, []

    def text_id(text):
        if text not in text_ids:
            text_ids[text] = len(texts)
            texts.append(text)
        return text_ids[text]

    for p in sorted(shipped(meta), key=lambda p: (p["name"].lower(), version_key(p["version"]))):
        files = [{"name": n, "text": text_id(t)} for n, t in license_files(p)]
        if not files:
            files = [{"name": "Cargo.toml", "text": text_id(
                f"The package has no license file. Its Cargo.toml gives the license {p['license']}."
                + (f"\n\nAuthors: {', '.join(p['authors'])}" if p.get("authors") else "")
            )}]
        crates.append({"name": p["name"], "version": p["version"], "license": p["license"] or "Unknown", "files": files})
    # Code and data in the program that have no crate of their own.
    crates.append({"name": "SQLite", "version": sqlite_version(meta), "license": "Public domain",
                   "files": [{"name": "sqlite.org/copyright", "text": text_id(SQLITE_NOTICE)}]})
    # Data in the program (assets/README.md).
    crates.append({"name": "UJI Pen Characters", "version": "2", "license": "CC-BY-4.0",
                   "files": [{"name": "assets/handwriting.bin", "text": text_id(UJI_NOTICE)}]})
    crates.sort(key=lambda c: (c["name"].lower(), version_key(c["version"])))
    return {
        "comment": "Made by scripts/licenses.py from cargo metadata. Do not edit.",
        "target": TARGET,
        "crates": crates,
        "texts": texts,
    }


def main():
    data = json.dumps(build(), indent=1, ensure_ascii=False) + "\n"
    if "--check" in sys.argv:
        with open(OUT, encoding="utf-8") as f:
            if f.read() != data:
                sys.exit("assets/licenses.json is not up to date: run python3 scripts/licenses.py")
        return
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(data)
    print(f"{OUT}: {len(json.loads(data)['crates'])} crates")


if __name__ == "__main__":
    main()
