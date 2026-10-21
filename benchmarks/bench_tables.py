"""Regenerate the result tables in docs/benchmarks.md from results.json.

The page's prose is written by hand; its nine result tables are not. They
were maintained by copy-paste, which is how some of them ended up showing
another test's numbers. This rewrites every table in place from
`benchmarks/results.json`, keyed by the test section, so a republish is
`python benchmarks/run.py && python benchmarks/bench_tables.py` and the
numbers cannot drift from the recorded run.

    python benchmarks/bench_tables.py [--check]

`--check` exits non-zero when the page is out of date (useful in CI or
before a commit) and rewrites nothing.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).parent
ROOT = HERE.parent
RESULTS = HERE / "results.json"
PAGE = ROOT / "docs" / "benchmarks.md"

# Section order in the page, and the first-column label for each row's
# sweep value. Kept explicit so a reordered page fails loudly instead of
# silently writing the wrong numbers.
SECTIONS = [
    ("scan_size", "Size", "{v} KB"),
    ("scan_position", "Needle at", "{v}%"),
    ("extract_size", "Size", "{v} KB"),
    ("extract_density", "Density", "{v} /KB"),
    ("pathological", "n", "{v}"),
    ("threads_gil", "Workers", "{v}"),
    ("threads_free", "Workers", "{v}"),
    ("threads_pos_gil", "Needle at", "{v}%"),
    ("threads_pos_free", "Needle at", "{v}%"),
]


def fmt_time(seconds: float | None) -> str:
    """Three significant digits in the largest unit that fits."""
    if seconds is None:
        return "—"
    for unit, scale in (("s", 1.0), ("ms", 1e-3), ("µs", 1e-6), ("ns", 1e-9)):
        if seconds >= scale:
            value = seconds / scale
            if value >= 100:
                return f"{value:.0f} {unit}"
            if value >= 10:
                return f"{value:.1f} {unit}"
            return f"{value:.2f} {unit}"
    return f"{seconds / 1e-9:.0f} ns"


def fmt_ratio(ratio: float) -> str:
    if ratio >= 1000:
        return f"{ratio:,.0f}"
    if ratio >= 100:
        return f"{ratio:.0f}"
    return f"{ratio:.1f}"


def fmt_speedup(re_s: float | None, rurex_s: float | None) -> str:
    if not re_s or not rurex_s:
        return "—"
    if rurex_s <= re_s:
        return f"rurex ×{fmt_ratio(re_s / rurex_s)}"
    return f"`re` ×{fmt_ratio(rurex_s / re_s)}"


def rows_for(sweep: dict, label: str) -> list[str]:
    out = []
    for point in sweep["points"]:
        re_s = point.get("re_s")
        rurex_s = point.get("rurex_s")
        first = label.format(v=point["value"])
        cells = [first]
        # The pathological test's last point has no `re` number: it was
        # killed at the timeout.
        if re_s is None:
            cells.append("*≥ 2 s — still running at the timeout*")
        else:
            cells.append(fmt_time(re_s))
        cells.append(fmt_time(rurex_s))
        cells.append(fmt_speedup(re_s, rurex_s))
        out.append("| " + " | ".join(cells) + " |")
    return out


def rewrite(text: str, tests: dict) -> tuple[str, list[str]]:
    lines = text.split("\n")
    out: list[str] = []
    problems: list[str] = []
    section = -1
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("## Test "):
            section += 1
        out.append(line)
        # A table starts with a header row and a separator row.
        if (
            section >= 0
            and section < len(SECTIONS)
            and line.startswith("| ")
            and i + 1 < len(lines)
            and lines[i + 1].startswith("|")
            and set(lines[i + 1]) <= set("|-: ")
        ):
            name, label, first_fmt = SECTIONS[section]
            sweep = tests[name]["sweeps"][0]
            out.append(lines[i + 1])
            i += 2
            # Skip the old data rows.
            while i < len(lines) and lines[i].startswith("| "):
                i += 1
            out.extend(rows_for(sweep, first_fmt))
            continue
        i += 1
    return "\n".join(out), problems


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    doc = json.loads(RESULTS.read_text())
    tests = {t["name"]: t for t in doc["tests"]}
    missing = [name for name, _, _ in SECTIONS if name not in tests]
    if missing:
        raise SystemExit(f"results.json is missing tests: {missing}")

    original = PAGE.read_text()
    updated, problems = rewrite(original, tests)
    if args.check:
        if updated != original or problems:
            print("docs/benchmarks.md is stale; run benchmarks/bench_tables.py")
            return 1
        print("docs/benchmarks.md tables are current")
        return 0
    if updated != original:
        PAGE.write_text(updated)
        print(f"rewrote the tables in {PAGE.relative_to(ROOT)}")
    else:
        print("tables already current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
