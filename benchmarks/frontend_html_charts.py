"""Generate the frontend HTML benchmark charts (PNG), the Markdown
results page, and the standalone HTML twin.

    python benchmarks/frontend_html_charts.py            # charts + .md + .html
    python benchmarks/frontend_html_charts.py --no-html  # skip the HTML twin

Numbers come from frontend_html_results.json (written by html.py); this file
only renders them.
"""

import base64
import json
import pathlib
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

try:
    import markdown  # pip install markdown
except ImportError:
    markdown = None

HERE = pathlib.Path(__file__).parent
DOCS = HERE.parent / "docs"
IMG = DOCS / "img"
RESULTS = HERE / "frontend_html_results.json"

COLOR_RE = "#4C72B0"
COLOR_RUREX = "#CE422B"

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

TITLES = {
    "gil": "Frontend HTML href/src — GIL Python",
    "free": "Frontend HTML href/src — free-threaded Python",
}

SERIES = (
    ("re", "str", COLOR_RE, "-", "o", "re / str"),
    ("re", "bytes", COLOR_RE, (0, (6, 3)), "s", "re / bytes"),
    ("rurex", "str", COLOR_RUREX, "-", "o", "rurex / str"),
    ("rurex", "bytes", COLOR_RUREX, (0, (6, 3)), "s", "rurex / bytes"),
)


def chart(data, interp, out_name):
    pts = [p for p in data["points"] if p["interpreter"] == interp]
    xs = sorted({p["threads"] for p in pts})
    fig, ax = plt.subplots(figsize=(9.5, 5.4))
    for engine, form, color, ls, marker, label in SERIES:
        ys = [
            next(
                p["wall_s"]
                for p in pts
                if p["threads"] == x
                and p["engine"] == engine
                and p["input"] == form
            )
            for x in xs
        ]
        ax.plot(
            xs,
            ys,
            marker=marker,
            linestyle=ls,
            color=color,
            linewidth=2.2,
            markersize=6,
            label=label,
            zorder=3,
        )
        ax.text(
            xs[-1],
            ys[-1],
            "  " + f"{ys[-1]:.2f} s",
            color=color,
            fontsize=9,
            va="center",
            ha="left",
        )
    ax.set_xticks(xs)
    ax.set_xlabel("worker threads")
    ax.set_ylim(bottom=0)
    ax.set_ylabel("wall time for the whole pile (s)")
    ax.set_title(TITLES[interp], loc="left", fontsize=13, fontweight="bold", pad=12)
    # Below the axes, left-aligned: on these panels the lower-left
    # corner is where the rurex lines live, so an in-plot legend would
    # cover them.
    ax.legend(
        loc="upper left",
        bbox_to_anchor=(0.0, -0.13),
        ncol=4,
        handlelength=3.4,
        handletextpad=0.7,
        columnspacing=1.8,
        frameon=False,
    )
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def table(data, interp):
    pts = [p for p in data["points"] if p["interpreter"] == interp]
    by = {(p["threads"], p["engine"], p["input"]): p["wall_s"] for p in pts}
    rows = []
    for x in sorted({p["threads"] for p in pts}):
        rstr = by[(x, "re", "str")]
        rbytes = by[(x, "re", "bytes")]
        ustr = by[(x, "rurex", "str")]
        ubytes = by[(x, "rurex", "bytes")]
        rows.append(
            f"| {x} | {rstr:.2f} s | {rbytes:.2f} s "
            f"| {ustr:.2f} s | {ubytes:.2f} s "
            f"| ×{ustr / rstr:.2f} | ×{ubytes / rbytes:.2f} |"
        )
    return (
        "| Workers | `re` str | `re` bytes | rurex str | rurex bytes "
        "| rurex/re str | rurex/re bytes |\n"
        "|---:|---:|---:|---:|---:|---:|---:|\n"
        + "\n".join(rows)
        + "\n\n*(ratio < 1 = rurex faster)*"
    )


def render_md():
    data = json.loads(RESULTS.read_text())
    s = data["stats"]
    n = s["documents"]
    total_mb = s["total_bytes"] / 1e6
    median_kb = s["median_document_bytes"] / 1024

    chart(data, "gil", "html_threads_gil.png")
    chart(data, "free", "html_threads_free.png")

    def w(interp, threads, engine, inp):
        for p in data["points"]:
            if (
                p["interpreter"] == interp
                and p["threads"] == threads
                and p["engine"] == engine
                and p["input"] == inp
            ):
                return p["wall_s"]
        raise KeyError((interp, threads, engine, inp))

    def x(interp, threads, inp="str"):
        return w(interp, threads, "rurex", inp) / w(interp, threads, "re", inp)

    md = f"""# Frontend HTML href/src extraction: rurex vs `re`

Real-world task: **one** pass of `.findall(document)` over real frontend
HTML pages. Each measured point is a fresh interpreter process that
loads the corpus, pins its workers to distinct physical cores, runs a
fixed CPU warm-up on a separate buffer, and then performs the single
pass split across N barrier-started workers; there is no repeat scan.

This is the long-haystack complement to the
[Telegram benchmark](telegram-benchmark.md): documents are large
(median {median_kb:.0f} KB), matches are dense
({s['median_matches']:.0f} per document, up to {s['max_matches']:,}),
and capture groups are part of the result.

## Methodology

- **Pattern** (quoted and unquoted HTML attribute values):
  `(?i)\\b(?:href|src)\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`
- **Corpus**: the owner-provided frontend HTML sample, 2,086 documents
  and 479.9 MiB decoded. The benchmark downloads
  `frontend-html-500mb-v1.tgz` from the
  [`bench-data-html-v1` release](https://github.com/lorien/rurex/releases/tag/bench-data-html-v1),
  sha256-verifies it, and caches a length-prefixed local copy. See
  [`benchmarks/html/PROVENANCE.md`](../benchmarks/html/PROVENANCE.md).
  The default run uses the first {total_mb:.0f} MiB of the corpus
  ({n:,} documents); `--full` loads all of it.
- **CPU placement and warm-up**: the process is pinned to distinct
  physical cores; free-threaded workers are pinned one per core, while
  GIL workers share the process affinity so the active thread does not
  bounce across cores. All workers run a fixed CPU-bound warm-up on a
  separate buffer before the timed barrier. The measured corpus is
  never touched by the warm-up. Governor, load average and per-CPU
  frequency are recorded in the raw results.
- **Core lease**: the whole matrix holds `/tmp/core4.lock` (mode
  0666). A second run prints the owner metadata, waits up to 600 s, and
  then fails instead of running on the same cores.
- **Contamination guard**: each point compares CPU time with wall time.
  GIL points use process CPU / wall; free-threaded points require every
  worker's CPU / wall to stay above `RUREX_MIN_EFFICIENCY` (default
  0.90). A point above `RUREX_MAX_LOAD` or below the efficiency floor is
  rejected and retried up to `RUREX_RETRIES` (default 2); repeated
  failure aborts the run.
- **Timing window**: from the instant the post-warm-up barrier releases
  to the instant the slice is finished; a point is the max across
  workers. Load and pattern compile are outside the window, identically
  for both engines. Every run's match count must equal
  {s['total_matches']:,}.
- **Repeats**: published rows are the median of three complete runs;
  each raw result records the individual wall times.
- **Interpreters**: GIL CPython 3.13 and free-threaded 3.14t, one fresh
  subprocess per point. CPU: {data['environment']['cpu']}.

## GIL Python

![Threads on GIL Python](img/html_threads_gil.png)

{table(data, "gil")}

Single-threaded, rurex answers the parsed-`str` form in
{w('gil', 1, 'rurex', 'str'):.2f} s against `re`'s
{w('gil', 1, 're', 'str'):.2f} s (×{x('gil', 1, 'str'):.2f}), and the
bytes form in {w('gil', 1, 'rurex', 'bytes'):.2f} s against
{w('gil', 1, 're', 'bytes'):.2f} s (×{x('gil', 1, 'bytes'):.2f}).

The `str` result is the opposite of the short-post Telegram regime.
Documents here have a median of {median_kb:.0f} KB, so the cold
UCS-to-UTF-8 transcode is amortized over far more text; the
required-first-byte prefilter still skips pages that cannot contain
`href` or `src`, and the AVX2 kernel handles the rest. The engine and
its SIMD prefilters then do the dominant work.

Threads reinforce the difference. These documents are above the 4 KB
GIL-release threshold, so rurex releases the GIL during the scan, and
source buffers of at least 32 KB also release it around the cold
transcode itself, so one worker's encode does not block the others:
`str` drops from {w('gil', 1, 'rurex', 'str'):.2f} to
{w('gil', 4, 'rurex', 'str'):.2f} s and bytes from
{w('gil', 1, 'rurex', 'bytes'):.2f} to
{w('gil', 4, 'rurex', 'bytes'):.2f} s at four workers. `re` stays flat
on `str` ({w('gil', 1, 're', 'str'):.2f} to
{w('gil', 4, 're', 'str'):.2f} s) because it never releases the GIL
during matching.

## Free-threaded Python

![Threads on free-threaded Python](img/html_threads_free.png)

{table(data, "free")}

Without the GIL both engines scale. rurex goes from
{w('free', 1, 'rurex', 'str'):.2f} to
{w('free', 4, 'rurex', 'str'):.2f} s on `str` and from
{w('free', 1, 'rurex', 'bytes'):.2f} to
{w('free', 4, 'rurex', 'bytes'):.2f} s on bytes; `re` goes from
{w('free', 1, 're', 'str'):.2f} to {w('free', 4, 're', 'str'):.2f} s
on `str` and from {w('free', 1, 're', 'bytes'):.2f} to
{w('free', 4, 're', 'bytes'):.2f} s on bytes. rurex keeps the lead at
every thread count. The `str` and bytes rows are close for rurex
because the transcode is a small part of a long-document pass.

## Reading the numbers

This page and the [Telegram page](telegram-benchmark.md) describe the
two ends of the real-world spectrum. Telegram is short non-ASCII posts
scanned once, where per-call transcode and per-match result building
dominate; rurex had to close that gap with a prefilter and a leaner
`findall`. Frontend HTML is long UTF-8 documents with dense, grouped
extraction, where the engine and its prefilters dominate; rurex leads
on both input forms and scales under the GIL once the documents cross
the release threshold.

Machine-specific; expect different absolute numbers elsewhere.

## Reproducing

```sh
python benchmarks/frontend_html.py && cp benchmarks/frontend_html_results.json run1.json
python benchmarks/frontend_html.py && cp benchmarks/frontend_html_results.json run2.json
python benchmarks/frontend_html.py && cp benchmarks/frontend_html_results.json run3.json
python benchmarks/merge_runs.py run1.json run2.json run3.json -o benchmarks/frontend_html_results.json
python benchmarks/frontend_html_charts.py     # charts, this page, and the HTML twin
```

---

*Raw results: [`benchmarks/frontend_html_results.json`](../benchmarks/frontend_html_results.json).
Numbers on this page were measured on 2026-10-18 with rurex 0.1.0;
see the raw results file for the exact environment.*
"""
    out = DOCS / "homepage-benchmark.md"
    out.write_text(md)
    return out


HTML_TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>rurex — frontend HTML benchmark</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI",
         Helvetica, Arial, sans-serif; max-width: 880px; margin: 2rem auto;
         padding: 0 1rem; color: #1f2328; line-height: 1.6; }}
  code {{ background: #f6f8fa; padding: .2em .4em; border-radius: 6px;
         font-size: 85%; font-family: ui-monospace, Menlo, Consolas,
         monospace; }}
  pre {{ background: #f6f8fa; padding: 14px 16px; border-radius: 8px; }}
  table {{ border-collapse: collapse; margin: 1rem 0; }}
  th, td {{ border: 1px solid #d1d9e0; padding: 6px 13px; }}
  th {{ background: #f6f8fa; }}
  img {{ max-width: 100%; }}
  h1, h2 {{ border-bottom: 1px solid #d1d9e0; padding-bottom: .3em; }}
</style>
</head>
<body>
<div class="banner" style="background:#fff8c5;border:1px solid #d4a72c;
     border-radius:8px;padding:.6rem 1rem;margin-bottom:2rem;font-size:90%%;">
Rendered from <code>docs/homepage-benchmark.md</code> with embedded
images — regenerate with <code>python benchmarks/frontend_html_charts.py</code>.</div>
{body}
</body>
</html>
"""


def render_html():
    if markdown is None:
        stale = DOCS / "homepage-benchmark.html"
        print(
            "WARNING: the markdown package is missing, so "
            f"{stale} was NOT regenerated and will keep showing the "
            "previous run's numbers. Fix with: pip install markdown",
            file=sys.stderr,
        )
        return None
    md_text = (DOCS / "homepage-benchmark.md").read_text()
    body = markdown.markdown(md_text, extensions=["tables", "fenced_code"])

    def embed(match):
        src = match.group(1)
        b64 = base64.b64encode((DOCS / src).read_bytes()).decode()
        return f'src="data:image/png;base64,{b64}"'

    body = re.sub(r'src="(img/[^"]+)"', embed, body)
    out = DOCS / "homepage-benchmark.html"
    out.write_text(HTML_TEMPLATE.format(body=body))
    return out


def main():
    IMG.mkdir(parents=True, exist_ok=True)
    print("wrote", render_md())
    if "--no-html" not in sys.argv:
        print("wrote", render_html())


if __name__ == "__main__":
    main()
