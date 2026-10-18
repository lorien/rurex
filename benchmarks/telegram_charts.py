"""Generate the Telegram benchmark charts (PNG), the Markdown results
page, and the standalone HTML twin.

    python benchmarks/telegram_charts.py            # charts + .md + .html
    python benchmarks/telegram_charts.py --no-html  # skip the HTML twin

The HTML twin is written by default because it embeds the images as
base64: it is a self-contained snapshot, so if it is *not* regenerated
it keeps showing the previous run's charts and numbers forever. Making
that opt-in caused exactly that confusion once — do not make it opt-in
again.

Numbers come from telegram_results.json (written by telegram.py);
this file only renders them.
"""

import base64
import json
import pathlib
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
RESULTS = HERE / "telegram_results.json"

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
    "gil": "Telegram @mentions — GIL Python",
    "free": "Telegram @mentions — free-threaded Python",
}

SERIES = (
    ("re", "str", "#4C72B0", "-", "o", "re / str"),
    ("re", "bytes", "#4C72B0", (0, (6, 3)), "s", "re / bytes"),
    ("rurex", "str", "#CE422B", "-", "o", "rurex / str"),
    ("rurex", "bytes", "#CE422B", (0, (6, 3)), "s", "rurex / bytes"),
)


def chart(data, interp, out_name):
    pts = [p for p in data["points"] if p["interpreter"] == interp]
    xs = sorted({p["threads"] for p in pts})
    fig, ax = plt.subplots(figsize=(9.5, 5.4))
    for engine, form, color, ls, marker, label in SERIES:
        ys = [next(p["wall_s"] for p in pts
                   if p["threads"] == x and p["engine"] == engine
                   and p["input"] == form)
              for x in xs]
        ax.plot(xs, ys, marker=marker, linestyle=ls, color=color,
                linewidth=2.2, markersize=6, label=label, zorder=3)
        ax.text(xs[-1], ys[-1], "  " + f"{ys[-1]:.2f} s",
                color=color, fontsize=9, va="center", ha="left")
    ax.set_xticks(xs)
    ax.set_xlabel("worker threads")
    ax.set_ylabel("wall time for the whole pile (s)")
    ax.set_title(TITLES[interp], loc="left", fontsize=13,
                 fontweight="bold", pad=12)
    ax.legend(loc="upper left", handlelength=4.2, handletextpad=0.8)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def table(data, interp, n_posts):
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
            f"| ×{ustr / rstr:.1f} | ×{ubytes / rbytes:.1f} |"
        )
    return ("| Workers | `re` str | `re` bytes | rurex str | rurex bytes "
            "| rurex/re str | rurex/re bytes |\n"
            "|---:|---:|---:|---:|---:|---:|---:|\n"
            + "\n".join(rows)
            + "\n\n*(ratio < 1 = rurex faster)*")


def render_md():
    data = json.loads(RESULTS.read_text())
    s = data["stats"]
    n = s["posts"]
    chart(data, "gil", "telegram_threads_gil.png")
    chart(data, "free", "telegram_threads_free.png")

    def w(interp, threads, engine, inp):
        """wall_s of one point — the prose quotes the table, never a
        hand-copied number (which goes stale on every re-run)."""
        for p in data["points"]:
            if (p["interpreter"] == interp and p["threads"] == threads
                    and p["engine"] == engine and p["input"] == inp):
                return p["wall_s"]
        raise KeyError((interp, threads, engine, inp))

    def x(interp, threads, inp="str"):
        return w(interp, threads, "rurex", inp) / w(interp, threads, "re", inp)

    md = f"""# Telegram @mention extraction: rurex vs `re`

Real-world task, measured exactly as performed: **one** pass of
`.findall(text)` over every post — one search, once, on a corpus that
has never been scanned before. Each measured point is a **fresh
interpreter process** that loads the corpus and performs the single
pass split across N barrier-started workers; there is no warm-up and
no repeat scan. This is the extraction-bound regime (short texts,
≈0.33 matches per post) — the opposite of the scan-bound workloads of
[the main benchmark page](benchmarks.md), where rurex leads ×7–13
single-threaded.

## Methodology

- **Pattern** (Telegram username rules — 5–32 chars,
  `[a-zA-Z0-9_]`, starts with a letter):
  `@[a-zA-Z][a-zA-Z0-9_]{{4,31}}`
- **Corpus**: {n:,} posts, {s['total_bytes'] / 1e6:.0f} MB of text
  (mostly Arabic/Persian), {s['total_mentions']:,} mentions in
  {s['posts_with_mentions']:,} posts ({100 * s['posts_with_mentions'] / n:.1f}%
  of posts). Downloaded automatically from the
  [`bench-data-telegram-v1` release](https://github.com/lorien/rurex/releases/tag/bench-data-telegram-v1)
  and sha256-verified; see
  [`benchmarks/telegram/PROVENANCE.md`](../benchmarks/telegram/PROVENANCE.md).
- **Timing window**: from the instant a worker passes the barrier to
  the instant its slice is finished; a point is the max across
  workers. Load and pattern compile are outside the window,
  identically for both engines. Every run's mention count must equal
  {s['total_mentions']:,}.
- **Interpreters**: GIL CPython 3.13 and free-threaded 3.14t, one
  fresh subprocess per point. CPU: {data['environment']['cpu']}.

## GIL Python

![Threads on GIL Python](img/telegram_threads_gil.png)

{table(data, "gil", n)}

Each point runs both input forms. The **str** variant is the real
task on parsed text: `re` wins it, because it never converts — the
haystack is UCS in memory and `re` matches it directly. rurex's
engine matches UTF-8, so a non-ASCII `str` must be transcoded first,
and that per-call transcode is the entire gap (the engine itself is
2.2× faster than `re` on the same bytes — see the bytes row). The
transcoder is a hand-written AVX2 kernel (ADR-0012): it converts at
roughly 1 GB/s effective on short scattered strings where CPython's
encoder and the scalar fallback manage ~0.45 GB/s, and strings whose
UTF-8 cache another C-API consumer already filled are borrowed
zero-copy instead. What remains is partly memory traffic on the
string's own bytes, which no faster encoder can remove — which is why
the row is still behind `re` despite the kernel being ~1.5× faster
than its first version.

The **bytes** variant feeds the same posts pre-encoded to UTF-8 —
the corpus is already UTF-8 on disk, so a pipeline that stays in
bytes never pays the transcode: rurex answers in
**{w('gil', 1, 'rurex', 'bytes'):.2f} s, beating `re`'s
{w('gil', 1, 're', 'bytes'):.2f} s (×{x('gil', 1, 'bytes'):.1f})**
single-threaded. That is the RE2-binding-recommended shape (feed
UTF-8 bytes), and rurex makes it a first-class mode.

With threads, rurex str stays flat
({w('gil', 1, 'rurex', 'str'):.2f}–{w('gil', 4, 'rurex', 'str'):.2f} s; an earlier build
degraded to 5.38 s before the 4 KB GIL-release threshold landed —
rurex releases the GIL only for ≥ 4 KB haystacks, matching `re`'s
own never-release-during-matching behavior for short texts), and
rurex bytes improves modestly ({w('gil', 1, 'rurex', 'bytes'):.2f} →
{w('gil', 4, 'rurex', 'bytes'):.2f} s at four workers: the scans are
sub-µs, so the GIL-held per-call result building dominates — the same
ceiling `re` bytes hits).

## Free-threaded Python

![Threads on free-threaded Python](img/telegram_threads_free.png)

{table(data, "free", n)}

Without the GIL both variants scale: rurex str
{w('free', 1, 'rurex', 'str'):.2f} s → {w('free', 4, 'rurex', 'str'):.2f} s
({w('free', 1, 'rurex', 'str') / w('free', 4, 'rurex', 'str'):.1f}×; the transcode
parallelizes like any other work) and rurex bytes
{w('free', 1, 'rurex', 'bytes'):.2f} s → {w('free', 4, 'rurex', 'bytes'):.2f} s;
`re` {w('free', 1, 're', 'str'):.2f} s → {w('free', 4, 're', 'str'):.2f} s str,
{w('free', 1, 're', 'bytes'):.2f} s → {w('free', 4, 're', 'bytes'):.2f} s bytes.
On this task free-threaded Python is the fastest way to run either
engine at every thread count, and rurex bytes at
{w('free', 4, 'rurex', 'bytes'):.2f} s is within
{x('free', 4, 'bytes'):.1f}× of the best `re` number while carrying
no transcode tax at all.

## Reading the numbers

Read the two variants as two different questions. **str**: what a
user scanning parsed text once pays — `re` wins (it never
transcodes); rurex pays a per-scan UCS→UTF-8 transcode (AVX2 where
available, scalar elsewhere; re-scans of strings warmed by another
C-API consumer borrow the cache and reach near-parity). **bytes**:
what a UTF-8-native pipeline pays — rurex **wins**
(×{x('gil', 1, 'bytes'):.1f} single-threaded), because its engine
scans UTF-8 directly with SIMD prefilters. The scan-bound leads on
the main page (×7–13) do not transfer to short-text per-call
extraction, but the bytes variant shows rurex ahead where the input
is already UTF-8 — which for a JSONL/log corpus is the natural shape.
Machine-specific;
expect different absolute numbers elsewhere.

## Reproducing

```sh
python benchmarks/telegram.py          # downloads the corpus on first run,
                                       # then 16 cold runs, ~5 min
python benchmarks/telegram_charts.py   # regenerates the charts, this page
                                       # and the standalone HTML twin
```

---

*Raw results: [`benchmarks/telegram_results.json`](../benchmarks/telegram_results.json).
Numbers on this page were measured on 2026-10-17 with rurex 0.1.0;
see the raw results file for the exact environment.*
"""
    out = DOCS / "telegram-benchmarks.md"
    out.write_text(md)
    return out


HTML_TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>rurex — Telegram @mention benchmark</title>
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
Rendered from <code>docs/telegram-benchmarks.md</code> with embedded
images — regenerate with <code>python benchmarks/telegram_charts.py</code>.</div>
{body}
</body>
</html>
"""


def render_html():
    if markdown is None:
        stale = DOCS / "telegram-benchmarks.html"
        print(
            "WARNING: the markdown package is missing, so "
            f"{stale} was NOT regenerated and will keep showing the "
            "previous run's numbers. Fix with: pip install markdown",
            file=sys.stderr,
        )
        return None
    md_text = (DOCS / "telegram-benchmarks.md").read_text()
    body = markdown.markdown(md_text, extensions=["tables", "fenced_code"])

    def embed(match):
        src = match.group(1)
        b64 = base64.b64encode((DOCS / src).read_bytes()).decode()
        return f'src="data:image/png;base64,{b64}"'

    import re

    body = re.sub(r'src="(img/[^"]+)"', embed, body)
    out = DOCS / "telegram-benchmarks.html"
    out.write_text(HTML_TEMPLATE.format(body=body))
    return out


def main():
    IMG.mkdir(parents=True, exist_ok=True)
    print("wrote", render_md())
    if "--no-html" not in sys.argv:
        print("wrote", render_html())


if __name__ == "__main__":
    main()
