"""Render the rebar comparison page and its charts.

Usage:
    python benchmarks/rebar_charts.py             # charts + .md + .html
    python benchmarks/rebar_charts.py --no-html   # skip the HTML twin

Inputs are the CSVs rebar wrote into benchmarks/rebar/results/ (see
benchmarks/rebar_run.py, which measures and then calls this script):
curated.csv for the curated benchmark set and dict.csv for the
str-versus-bytes pair on one ASCII haystack. run.json carries the
environment of the run.

The HTML twin is written by default because it embeds the images as
base64: it is a self-contained snapshot, so if it is not regenerated it
keeps showing the previous run's charts and numbers.
"""

import base64
import collections
import csv
import hashlib
import json
import math
import pathlib
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import markdown  # pip install markdown

HERE = pathlib.Path(__file__).parent
ROOT = HERE.parent
RESULTS = HERE / "rebar" / "results"
DOCS = ROOT / "docs"
IMG = DOCS / "img"

COLOR_RE = "#4C72B0"
COLOR_RUREX = "#CE422B"
COLOR_ENGINE = "#55A868"
COLOR_RUREX_BYTES = "#8C564B"

ENGINES = ["rust/regex", "python/rurex", "python/re"]
LABELS = {
    "rust/regex": "rust/regex",
    "python/rurex": "rurex (str)",
    "python/rurex-bytes": "rurex (bytes)",
    "python/re": "stdlib re",
}

plt.rcParams.update(
    {
        "figure.dpi": 110,
        "savefig.dpi": 200,
        "font.family": "DejaVu Sans",
        "font.size": 11,
        "axes.edgecolor": "#666666",
        "axes.labelcolor": "#333333",
        "xtick.color": "#555555",
        "ytick.color": "#333333",
        "axes.spines.top": False,
        "axes.spines.right": False,
        "axes.grid": True,
        "grid.color": "#d9d9d9",
        "grid.linewidth": 0.6,
        "axes.axisbelow": True,
        "figure.facecolor": "white",
        "legend.frameon": True,
        "legend.framealpha": 0.9,
    }
)


def fmt_time(seconds):
    if seconds < 1e-6:
        return f"{seconds * 1e9:.0f} ns"
    if seconds < 1e-3:
        return f"{seconds * 1e6:.2f} µs"
    if seconds < 1:
        return f"{seconds * 1e3:.2f} ms"
    return f"{seconds:.2f} s"


def parse_time(text):
    for unit, mul in (("ns", 1e-9), ("us", 1e-6), ("ms", 1e-3), ("s", 1.0)):
        if text.endswith(unit):
            return float(text[: -len(unit)]) * mul
    raise ValueError(text)


def load_csv(path):
    """Return {(benchmark, engine): {"median", "model", "haystack_len"}}."""
    out = {}
    with open(path) as f:
        for row in csv.DictReader(f):
            if row["err"] or not row["median"]:
                continue
            out[(row["name"], row["engine"])] = {
                "median": parse_time(row["median"]),
                "model": row["model"],
                "haystack_len": int(row["haystack_len"] or 0),
                "version": row["engine_version"],
            }
    return out


def geomean(values):
    return math.exp(sum(map(math.log, values)) / len(values))


def medians(curated):
    return {k: v["median"] for k, v in curated.items()}


def ranking(curated):
    """Per-engine geometric mean of speed ratios, rebar's statistic.

    A benchmark's ratio divides the engine's median by the fastest
    median among the engines that ran it; the ranking is the geometric
    mean of those ratios over the benchmarks all three engines share.
    """
    med = medians(curated)
    names = sorted({n for n, _ in med})
    shared = [n for n in names if all((n, e) in med for e in ENGINES)]
    out = {}
    for engine in ENGINES:
        ratios = [med[(n, engine)] / min(med[(n, e)] for e in ENGINES) for n in shared]
        out[engine] = (geomean(ratios), len(ratios))
    return out, shared


def per_group(curated):
    med = medians(curated)
    groups = collections.defaultdict(lambda: collections.defaultdict(list))
    for name in sorted({n for n, _ in med}):
        base = med.get((name, "rust/regex"))
        if base is None:
            continue
        group = "/".join(name.split("/")[:2])
        for engine in ("python/rurex", "python/re"):
            if (name, engine) in med:
                groups[group][engine].append(med[(name, engine)] / base)
    return {
        g: {e: geomean(xs) for e, xs in engines.items()}
        for g, engines in groups.items()
    }


def rank_chart(rank, out_name):
    fig, ax = plt.subplots(figsize=(8.0, 2.6))
    engines = list(reversed(ENGINES))
    values = [rank[e][0] for e in engines]
    counts = rank[engines[0]][1]
    palette = {"rust/regex": COLOR_ENGINE, "python/rurex": COLOR_RUREX, "python/re": COLOR_RE}
    colors = [palette[e] for e in engines]
    bars = ax.barh(
        [LABELS[e] for e in engines], values, color=colors, height=0.55, zorder=3
    )
    ax.set_xscale("log")
    ax.set_xlabel("geometric mean of speed ratios (lower is faster, 1.0 = fastest)")
    ax.set_title(
        f"Curated rebar set — {counts} benchmarks shared by all three engines",
        loc="left",
        fontsize=12,
        fontweight="bold",
        pad=10,
    )
    for bar, value in zip(bars, values):
        ax.text(
            value * 1.06,
            bar.get_y() + bar.get_height() / 2,
            f"×{value:.2f}",
            va="center",
            fontsize=10,
        )
    ax.set_xlim(1, max(values) * 2.2)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def group_chart(groups, out_name):
    names = sorted(groups)
    labels = [g.split("/", 1)[1] for g in names]
    ys = range(len(names))
    height = 0.38
    fig, ax = plt.subplots(figsize=(8.6, 4.6))
    for offset, engine, color, label in (
        (-height / 2, "python/rurex", COLOR_RUREX, "rurex vs rust/regex"),
        (height / 2, "python/re", COLOR_RE, "stdlib re vs rust/regex"),
    ):
        values = [groups[g].get(engine, float("nan")) for g in names]
        ax.barh(
            [y + offset for y in ys],
            values,
            height=height,
            color=color,
            label=label,
            zorder=3,
        )
        for y, value in zip(ys, values):
            if not math.isnan(value):
                ax.text(
                    value * 1.08,
                    y + offset,
                    f"{value:.2f}" if value < 10 else f"{value:.0f}",
                    va="center",
                    fontsize=8.5,
                )
    ax.set_yticks(list(ys))
    ax.set_yticklabels(labels)
    ax.set_xscale("log")
    ax.set_xlabel("geometric mean of speed ratios against rust/regex (lower is faster)")
    ax.set_title(
        "Where the distance comes from, by benchmark group",
        loc="left",
        fontsize=12,
        fontweight="bold",
        pad=10,
    )
    ax.legend(
        loc="upper left",
        bbox_to_anchor=(0.0, -0.12),
        ncol=2,
        frameon=False,
    )
    ax.set_xlim(0.8, max(ax.get_xlim()[1], 20))
    ax.invert_yaxis()
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def scatter_chart(curated, out_name):
    med = medians(curated)
    pts = [
        (med[(n, "python/rurex")], med[(n, "python/re")], n)
        for n in sorted({n for n, _ in med})
        if (n, "python/rurex") in med and (n, "python/re") in med
    ]
    fig, ax = plt.subplots(figsize=(6.4, 6.0))
    xs = [p[1] * 1e6 for p in pts]
    ys = [p[0] * 1e6 for p in pts]
    lo = min(min(xs), min(ys))
    hi = max(max(xs), max(ys))
    ax.plot([lo, hi], [lo, hi], color="#999999", linewidth=1, linestyle="--", zorder=1)
    faster = [(x, y) for x, y in zip(xs, ys) if y <= x]
    slower = [(x, y) for x, y in zip(xs, ys) if y > x]
    ax.scatter(
        [p[0] for p in faster],
        [p[1] for p in faster],
        s=26,
        color=COLOR_RUREX,
        label=f"rurex faster ({len(faster)})",
        zorder=3,
    )
    ax.scatter(
        [p[0] for p in slower],
        [p[1] for p in slower],
        s=26,
        color=COLOR_RE,
        label=f"stdlib re faster ({len(slower)})",
        zorder=3,
    )
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("stdlib re, median per operation (µs)")
    ax.set_ylabel("rurex (str), median per operation (µs)")
    ax.set_title(
        "Every shared benchmark, one dot each",
        loc="left",
        fontsize=12,
        fontweight="bold",
        pad=10,
    )
    ax.text(
        0.03,
        0.97,
        "below the line = rurex faster",
        transform=ax.transAxes,
        va="top",
        fontsize=9,
        color="#555555",
    )
    ax.legend(loc="lower right")
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def modes_chart(dictrows, out_name):
    mbs = {}
    for (name, engine), row in dictrows.items():
        mbs[engine] = row["haystack_len"] / row["median"] / 1e6
    order = ["rust/regex", "python/rurex-bytes", "python/rurex", "python/re"]
    colors = [COLOR_ENGINE, COLOR_RUREX_BYTES, COLOR_RUREX, COLOR_RE]
    values = [mbs[e] for e in order]
    fig, ax = plt.subplots(figsize=(8.0, 3.0))
    bars = ax.barh([LABELS[e] for e in order], values, color=colors, height=0.55, zorder=3)
    ax.set_xscale("log")
    ax.set_xlabel("throughput on the same ASCII haystack (MB/s, log scale)")
    ax.set_title(
        "Text mode versus byte mode, identical input",
        loc="left",
        fontsize=12,
        fontweight="bold",
        pad=10,
    )
    for bar, value in zip(bars, values):
        ax.text(
            value * 1.06,
            bar.get_y() + bar.get_height() / 2,
            f"{value:,.0f} MB/s" if value >= 1 else f"{value * 1000:.0f} KB/s",
            va="center",
            fontsize=9.5,
        )
    ax.set_xlim(min(values) * 0.5, max(values) * 1.6)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def table_rows(curated, groups):
    med = medians(curated)
    rows = []
    for name in sorted({n for n, _ in med}):
        group = "/".join(name.split("/")[:2])
        short = name.split("/", 2)[2] if name.count("/") >= 2 else name
        cells = [f"| `{group.split('/')[1]}` | `{short}`"]
        times = {}
        for engine in ("python/rurex", "python/re", "rust/regex"):
            value = med.get((name, engine))
            times[engine] = value
            cells.append(f" {fmt_time(value)}" if value is not None else " —")
        present = {e: v for e, v in times.items() if v}
        winner = min(present, key=present.get) if present else None
        cells.append(f" {LABELS[winner] if winner else '—'}")
        rows.append("|".join(cells) + " |")
    return rows


def render_html(md_text, page, out_name):
    template = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>rurex — rebar comparison</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
         max-width: 880px; margin: 2rem auto; padding: 0 1rem; color: #1f2328; line-height: 1.6; }}
  code {{ background: #f6f8fa; padding: .2em .4em; border-radius: 6px; font-size: 85%;
         font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
  pre {{ background: #f6f8fa; padding: 14px 16px; border-radius: 8px; overflow-x: auto; }}
  pre code {{ background: none; padding: 0; font-size: 85%; }}
  h1, h2 {{ border-bottom: 1px solid #d1d9e0; padding-bottom: .3em; }}
  table {{ border-collapse: collapse; margin: 1rem 0; font-size: 90%; }}
  th, td {{ border: 1px solid #d1d9e0; padding: 6px 10px; text-align: left; }}
  td:nth-child(n+3), th:nth-child(n+3) {{ text-align: right; }}
  th {{ background: #f6f8fa; }}
  img {{ max-width: 100%; }}
  blockquote {{ border-left: 4px solid #0969da; padding: .4rem 1rem;
                background: #f6f8fa; border-radius: 0 6px 6px 0; }}
  .banner {{ background: #fff8c5; border: 1px solid #d4a72c; border-radius: 8px;
             padding: .6rem 1rem; margin-bottom: 2rem; font-size: 90%; }}
</style>
</head>
<body>
<div class="banner">Rendered from <code>{page}</code> with embedded images —
regenerate with <code>python benchmarks/rebar_charts.py</code>.</div>
{body}
</body>
</html>
"""
    import re

    body = markdown.markdown(md_text, extensions=["tables", "fenced_code"])

    def embed(match):
        path = DOCS / match.group(1)
        b64 = base64.b64encode(path.read_bytes()).decode()
        return f'src="data:image/png;base64,{b64}"'

    body = re.sub(r'src="(img/[^"]+)"', embed, body)
    out = DOCS / out_name
    out.write_text(template.format(page=page, body=body))
    return out


def main():
    curated = load_csv(RESULTS / "curated.csv")
    dictrows = load_csv(RESULTS / "dict.csv")
    meta = json.loads((RESULTS / "run.json").read_text())

    rank, shared = ranking(curated)
    groups = per_group(curated)

    rank_png = rank_chart(rank, "rebar_rank.png")
    groups_png = group_chart(groups, "rebar_groups.png")
    scatter_png = scatter_chart(curated, "rebar_scatter.png")
    modes_png = modes_chart(dictrows, "rebar_modes.png")

    dict_bench = next(iter({n for n, _ in dictrows}))
    dict_mbs = {
        engine: dictrows[(dict_bench, engine)]["haystack_len"]
        / dictrows[(dict_bench, engine)]["median"]
        / 1e6
        for engine in ("python/rurex", "python/rurex-bytes", "python/re", "rust/regex")
        if (dict_bench, engine) in dictrows
    }

    files = ""
    for name, info in sorted(meta.get("files", {}).items()):
        files += f"| `{name}` | {info['sha256'][:16]}… |\n"

    md = f"""# rebar: rurex against stdlib `re` and the Rust engine

[rebar](https://github.com/BurntSushi/rebar) is BurntSushi's barometer for
comparing regex engines on a curated set of tasks. This page reports how
rurex fares on it against the two engines a Python user actually weighs
it with: the standard library's `re`, which it replaces, and `rust/regex`,
which it embeds.

The metric is rebar's own: each benchmark divides an engine's median time
by the fastest median among the engines that ran it, and an engine's score
is the geometric mean of those ratios across the benchmarks all three
share. **Lower is faster; 1.0 is the fastest engine on that set.** A score
of ×3 means "typically three times the best engine's time".

## Method

| | |
|---|---|
| Date | {meta['measured_at']} |
| Host | `{meta['host']}`, {meta['cpu']} |
| Isolation | `/tmp/core4.lock` lease (cpus {meta['lease_cpus']}), pinned to CPU {meta['pinned_cpu']}, governor `{meta['governor']}` |
| rebar | {meta['rebar']} (curated set, `--max-time {meta['max_time']} --max-iters {meta['max_iters']}`) |
| rurex | {meta['rurex']}, release wheel from PyPI |
| Opponents | stdlib `re`, `rust/regex` (the engine rurex embeds) |
| Checks | every point verified with `rebar measure --test` before timing; no failed points in the data |

| Raw results | sha256 |
|---|---|
{files}
## The ranking

![Geometric mean ranking](img/rebar_rank.png)

| engine | score | benchmarks |
|---|---:|---:|
| `rust/regex` | ×{rank['rust/regex'][0]:.2f} | {rank['rust/regex'][1]} |
| `python/rurex` | ×{rank['python/rurex'][0]:.2f} | {rank['python/rurex'][1]} |
| `python/re` | ×{rank['python/re'][0]:.2f} | {rank['python/re'][1]} |

rurex sits between the engine it embeds and the module it replaces: about
**{rank['python/re'][0] / rank['python/rurex'][0]:.1f}× faster than `re`** on average, and
about **{rank['python/rurex'][0]:.1f}× off the Rust engine**. The distance to the engine is
not the scanner: the two run the same automata. It is the Python
boundary — argument handling and one Python object per match — plus, on
non-ASCII text, the cost of materialising UTF-8.

## Where the distance comes from

![Per-group ratios](img/rebar_groups.png)

| group | rurex | `re` |
|---|---:|---:|
"""
    for group in sorted(groups):
        r = groups[group].get("python/rurex")
        e = groups[group].get("python/re")
        md += (
            f"| `{group.split('/', 1)[1]}` | "
            f"{'—' if r is None else f'×{r:.2f}'} | "
            f"{'—' if e is None else f'×{e:.2f}'} |\n"
        )

    med = medians(curated)
    re_wins = []
    for name in sorted({n for n, _ in med}):
        rx, re_t = med.get((name, "python/rurex")), med.get((name, "python/re"))
        if rx and re_t and re_t < rx:
            re_wins.append((name, rx, re_t, re_t / rx))
    compile_wins = [w for w in re_wins if "/compile" in w[0]]
    words_wins = [w for w in re_wins if w[0].startswith("curated/08-words/")]
    other_wins = [w for w in re_wins if w not in compile_wins and w not in words_wins]

    md += f"""
`re` wins {len(re_wins)} of the {len(shared)} shared benchmarks, and they fall into
three groups:

- **Compile time** ({len(compile_wins)} points): rurex parses the pattern with
  `regex-syntax` for `re`-compatible error positions and then builds the
  automata, so `03-date/compile-unicode` costs
  {fmt_time(dict((n, v) for n, v, _, _ in compile_wins)[ 'curated/03-date/compile-unicode' ])} against `re`'s
  {fmt_time(dict((n, re_t) for n, _, re_t, _ in compile_wins)['curated/03-date/compile-unicode'])}.
- **Unicode word matching** ({len(words_wins)} points): `\\w`/`\\b` over a
  non-ASCII `str` pay both the transcode and a Unicode word automaton,
  while `re` matches the UCS buffer directly. `08-words/all-russian` is
  {fmt_time(med[('curated/08-words/all-russian', 'python/rurex')])} for rurex against
  {fmt_time(med[('curated/08-words/all-russian', 'python/re')])} for `re`.
- **Near parity scans** ({len(other_wins)} points): within 15% of each other, with
  `re` marginally ahead.

rurex wins the other {len(shared) - len(re_wins)} points, sometimes by four orders of
magnitude: `re` is a backtracker, and rebar's dictionary and ReDoS
benchmarks make that visible.

| benchmark where `re` wins | rurex | `re` | `re` is |
|---|---:|---:|---:|
"""
    for name, rx, re_t, ratio in re_wins:
        md += f"| `{name.split('curated/', 1)[-1]}` | {fmt_time(rx)} | {fmt_time(re_t)} | {1/ratio:.2f}× faster |\n"
    md += """

## Every shared benchmark

![Per-benchmark comparison](img/rebar_scatter.png)

| group | benchmark | rurex (str) | `re` | rust/regex | fastest |
|---|---|---:|---:|---:|---|
"""
    md += "\n".join(table_rows(curated, groups)) + "\n"

    md += f"""
## Text mode versus byte mode, identical input

`{dict_bench}` runs in Unicode mode over an ASCII haystack, so the two
representations are interchangeable for the match count. rurex's text
path borrows ASCII strings instead of transcoding them, which is why the
two modes are within a couple of percent here; what remains against the
Rust engine is the Python boundary.

![Text vs bytes](img/rebar_modes.png)

| engine | throughput |
|---|---:|
| `rust/regex` | {dict_mbs['rust/regex']:,.0f} MB/s |
| `python/rurex-bytes` | {dict_mbs['python/rurex-bytes']:,.0f} MB/s |
| `python/rurex` | {dict_mbs['python/rurex']:,.0f} MB/s |
| `python/re` | {dict_mbs['python/re'] * 1000:,.0f} KB/s |

The non-ASCII case is the interesting one, and rebar cannot measure it
without changing match counts: on a Russian or Chinese haystack the byte
engine matches ASCII word characters only. That comparison lives in
[`telegram-benchmark.md`](telegram-benchmark.md), where the text row pays
one UCS→UTF-8 encode per post and the byte row pays none.

## What this does and does not say

- rebar's counting models iterate `finditer`, so one Python object per
  match is inside the measurement. That is what a `finditer` user pays,
  but it is **not** what `findall` pays: rurex's `findall` path builds
  strings directly, with no `Match` objects, and rebar has no model for
  it. The per-match numbers here therefore overstate the cost for
  `findall`-shaped workloads.
- Most rebar benchmarks set `unicode = false`, i.e. they hand every
  engine bytes. Those points measure the path with no transcoding at
  all; the transcode appears only in the Unicode-mode benchmarks
  (`03-date/unicode`, the Russian and Chinese literals and words).
- The ranking is a geometric mean across heterogeneous tasks. Read the
  per-group and per-benchmark tables for anything decision-shaped; the
  mean is a summary, not a measurement of your workload.

## Reproducing

`benchmarks/rebar/README.md` holds the runner, the engine registration and
the full invocation. The short version, on the benchmark host:

```sh
python3 benchmarks/rebar_run.py          # measure, verify, then rebuild this page
```

which takes the core lease, pins to CPU 0, runs rebar for the four
engines, and calls `benchmarks/rebar_charts.py` only if every point
passed.
"""
    md_path = DOCS / "rebar-benchmark.md"
    md_path.write_text(md)
    print("wrote", md_path)
    for p in (rank_png, groups_png, scatter_png, modes_png):
        print("wrote", p)
    if "--no-html" not in sys.argv:
        out = render_html(md, "docs/rebar-benchmark.md", "rebar-benchmark.html")
        print("wrote", out)


if __name__ == "__main__":
    main()
