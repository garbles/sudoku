#!/usr/bin/env python3
"""Compares a colors.json of the Sudoku (colors) app with the colors in src/settings.rs.

Usage: compare.py [COLORS_JSON] [SETTINGS_RS]
Exit status: 0 when the colors are different, 2 when they are the same, 1 for an error.
"""
import json
import re
import sys


def fail(message):
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)


def to_hex(rgb):
    return "#{:02X}{:02X}{:02X}".format(*rgb)


def valid(rgb):
    return isinstance(rgb, list) and len(rgb) == 3 and all(type(v) is int and 0 <= v <= 255 for v in rgb)


def read_source(path):
    text = open(path, encoding="utf-8").read()
    order = re.search(r"pub const ALL: \[Color; \d+\] = \[(.*?)\];", text, re.S)
    if not order:
        fail(f"{path}: Color::ALL is not found")
    names = re.findall(r"Color::(\w+)", order.group(1))
    values = {
        name: [int(v, 16) for v in parts]
        for name, *parts in re.findall(r"Color::(\w+) => \[0x(\w\w), 0x(\w\w), 0x(\w\w)\]", text)
    }
    number = re.search(r"pub const ENTRY_DIGIT: \[u8; 3\] = \[0x(\w\w), 0x(\w\w), 0x(\w\w)\]", text)
    if not number or any(name not in values for name in names):
        fail(f"{path}: the color values are not found")
    return names, values, [int(v, 16) for v in number.groups()]


def main():
    json_path = sys.argv[1] if len(sys.argv) > 1 else "target/colors/colors.json"
    source_path = sys.argv[2] if len(sys.argv) > 2 else "src/settings.rs"
    names, source, source_number = read_source(source_path)
    try:
        saved = json.load(open(json_path, encoding="utf-8"))
    except (OSError, ValueError) as e:
        fail(f"{json_path}: {e}")
    palette, number = saved.get("palette"), saved.get("number")
    if not isinstance(palette, list) or len(palette) != len(names) or not all(map(valid, palette)) or not valid(number):
        fail(f"{json_path}: the file does not have {len(names)} palette colors and a number color")

    rows = [(name, source[name], rgb) for name, rgb in zip(names, palette)]
    rows.append(("Your number", source_number, number))
    changed = 0
    print(f"{'Color':<12} {'Now':<8} {'New':<8}")
    for name, now, new in rows:
        mark = "" if now == new else "changed"
        changed += now != new
        print(f"{name:<12} {to_hex(now):<8} {to_hex(new):<8} {mark}".rstrip())

    seen = {}
    for name, rgb in zip(names, palette):
        seen.setdefault(to_hex(rgb), []).append(name)
    for value, same in seen.items():
        if len(same) > 1:
            print(f"caution: {', '.join(same)} have the same value {value}")
    if number[0] != number[1] or number[1] != number[2]:
        print(f"caution: Your number {to_hex(number)} is not a gray")
    white = names.index("White")
    if palette[white] != source["White"]:
        print(f"caution: White changes to {to_hex(palette[white])}")
    print(f"{changed} of {len(rows)} colors changed")
    sys.exit(0 if changed else 2)


if __name__ == "__main__":
    main()
