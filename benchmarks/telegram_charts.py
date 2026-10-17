"""Generate docs/telegram-benchmarks.html from telegram_results.json.

Standalone HTML: charts embedded as base64 PNGs, tables, methodology
and data-provenance sections. The document is committed.

    python benchmarks/telegram_charts.py
"""

import base64
import io
import json
import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

HERE = pathlib.Path(__file__).parent
RESULTS = HERE / "telegram_results.json"
OUT = HERE.parent / "docs" / "telegram-benchmarks.html"

COLOR_RE = "#4C72B0"
COLOR_RUREX = "#CE422B"

plt.rcParams.update({
    "figure.dpi": 110, "savefig.dpi": 200,
    "font.family": "DejaVu Sans", "font.size": 11,
    "axes.edgecolor": "#666666", "axes.labelcolor": "#333333",
    "xtick.color": "#555555", "ytick.color": "#333333",
    "axes.spines.top": False, "axes.spines.right": False,
    "axes.grid": True, "grid.color": "#d9d9d9", "grid.linewidth": 0.6,
    "axes.axisbelow": True, "figure.facecolor": "white",
    "legend.frameon": True, "legend.framealpha": 0.9,
})

TITLES = {
    "gil": "GIL Python 3.13 — one cold run per point",
    "free": "Free-threaded 3.14t — one cold run per point",
}


def section(data, interp):
    pts = [p for p in data["points"] if p["interpreter"] == interp]
    xs = sorted({p["threads"] for p in pts})
    fig, ax = plt.subplots(figsize=(8.6, 5.0))
    for engine, color, label in (("re", COLOR_RE, "re (stdlib)"),
                                  ("rurex", COLOR_RUREX, "rurex")):
        ys = [next(p["wall_s"] for p in pts
                   if p["threads"] == x and p["engine"] == engine)
              for x in xs]
        ax.plot(xs, ys, marker="o", color=color, linewidth=2.2,
                markersize=6, label=label, zorder=3)
        ax.text(xs[-1], ys[-1], f"  {ys[-1]:.2f} s", color=color,
                fontsize=9, va="center", ha="left")
    ax.set_xticks(xs)
    ax.set_xlabel("worker threads")
    ax.set_ylabel("wall time for the whole pile (s)")
    ax.set_title(TITLES[interp], loc="left", fontsize=13,
                 fontweight="bold", pad=10)
    ax.legend(loc="upper left")
    fig.tight_layout()
    buf = io.BytesIO()
    fig.savefig(buf, format="png", bbox_inches="tight")
    plt.close(fig)
    return base64.b64encode(buf.getvalue()).decode()


def table(data, interp, n_posts):
    pts = [p for p in data["points"] if p["interpreter"] == interp]
    by = {(p["threads"], p["engine"]): p["wall_s"] for p in pts}
    rows = []
    for x in sorted({p["threads"] for p in pts}):
        re_s = by[(x, "re")]
        ru_s = by[(x, "rurex")]
        rows.append(
            f"<tr><td class='num'>{x}</td>"
            f"<td class='num'>{re_s:.2f} s</td>"
            f"<td class='num'>{re_s * 1e6 / n_posts:.2f} µs</td>"
            f"<td class='num'>{ru_s:.2f} s</td>"
            f"<td class='num'>{ru_s * 1e6 / n_posts:.2f} µs</td>"
            f"<td class='num'>re ×{ru_s / re_s:.2f}</td></tr>"
        )
    return ("<table><thead><tr><th>Threads</th><th>re</th>"
            "<th>re, per post</th><th>rurex</th><th>rurex, per post</th>"
            f"<th>Speedup</th></tr></thead><tbody>{''.join(rows)}</tbody></table>")


def main():
    data = json.loads(RESULTS.read_text())
    stats = data["stats"]
    n = stats["posts"]
    charts = {i: section(data, i) for i in ("gil", "free")}
    free_html = (
        f"<h2>Free-threaded Python</h2>\n"
        f"<img alt='free-threaded chart' src='data:image/png;base64,"
        f"{charts['free']}'/>\n{table(data, 'free', n)}\n"
    ) if any(p["interpreter"] == "free" for p in data["points"]) else ""

    html = f"""<!DOCTYPE html>
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
  table {{ border-collapse: collapse; margin: 1rem 0; }}
  th, td {{ border: 1px solid #d1d9e0; padding: 6px 13px; }}
  th {{ background: #f6f8fa; }}
  td.num, th.num {{ text-align: right; }}
  img {{ max-width: 100%; }}
  h2 {{ border-bottom: 1px solid #d1d9e0; padding-bottom: .3em; }}
</style>
</head>
<body>

<h1>rurex — Telegram @mention extraction benchmark</h1>

<p>Real-world task, measured exactly as performed: <b>one</b> pass of
<code>findall</code> over every post's text — one search, once, on a
corpus that has never been scanned before. Each measured point is a
<b>fresh interpreter process</b> that loads the corpus and performs the
single pass split across N barrier-started workers. There is no
warm-up and no repeat scan; nothing is measured twice.</p>

<ul>
<li><b>Pattern</b> (Telegram username rules — 5–32 chars,
<code>[a-zA-Z0-9_]</code>, starts with a letter):
<code>@[a-zA-Z][a-zA-Z0-9_]{{4,31}}</code></li>
<li><b>Corpus</b>: {n:,} posts, {stats['total_bytes'] / 1e6:.0f} MB of
text (mostly Arabic/Persian), {stats['total_mentions']:,} mentions in
{stats['posts_with_mentions']:,} posts ({100 * stats['posts_with_mentions'] / n:.1f}%
of posts, ≈0.33 mentions per post). Provenance and SHA-256:
<code>benchmarks/telegram/PROVENANCE.md</code>.</li>
<li><b>Timing window</b>: from the instant a worker passes the barrier
to the instant its slice is finished; the point is the max across
workers. JSONL load and pattern compile are outside the window,
identically for both engines. Verified by a mention-count checksum
({stats['total_mentions']:,} per run).</li>
<li><b>Interpreters</b>: GIL CPython 3.13 and free-threaded 3.14t,
each as fresh subprocesses. CPU: {data['environment']['cpu']}.</li>
</ul>

<h2>GIL Python</h2>
<img alt="GIL chart" src="data:image/png;base64,{charts['gil']}"/>
{table(data, 'gil', n)}

<p>Single-threaded, <code>re</code> answers the whole pile in 0.48 s
and rurex in 1.65 s. Most of rurex's gap is <b>CPython's first-touch
UTF-8 materialization</b>: CPython stores <code>str</code> in a
UCS-1/2/4 form and lazily encodes-and-caches a UTF-8 copy inside each
string the first time anything asks for it — which is exactly what
rurex (like any UTF-8 engine consuming Python strings) must do, via
<code>PyUnicode_AsUTF8AndSize</code>. <code>re</code> never converts
— its engine walks the native form — so it pays nothing. Over 470 MB
of mostly non-ASCII text that is a one-time ~1 s encode, inside the
measured run because a scan-once user pays it.</p>

<p>Adding threads makes rurex's cold scan <b>worse</b> on the GIL
build (1.65 s → 5.38 s at four workers): the per-string UTF-8
allocation happens in rurex's GIL-held section, and the pile performs
a million detach/attach cycles around the scans — a GIL convoy whose
overhead grows with the thread count. <code>re</code> is roughly flat
(0.48–0.66 s).</p>

{free_html}
<p>Without the GIL the same cold scan scales cleanly: rurex 1.46 s →
0.52 s at four workers (2.8×) — the first-touch UTF-8 encoding
parallelizes like any other work — and <code>re</code> 0.42 s →
0.33 s. On this scan-once task free-threaded Python is the fastest
way to run rurex at every thread count.</p>

<h2>Reading the numbers</h2>

<p>This is the extraction-bound regime — short texts, ≈0.33 matches
per post, per-call overhead dominating — the opposite of the
scan-bound workloads on <a href="benchmarks.md">the main benchmark
page</a>, where rurex leads by 7–13× single-threaded. Here
<code>re</code> wins every point: partly per-call fixed cost, and
partly the one-time UTF-8 tax that only the UTF-8 engine pays. Users
who scan each text exactly once pay it once; users re-scanning
re-use the cached UTF-8 form and the tax vanishes. Machine-specific;
expect different absolute numbers elsewhere.</p>

<p>Reproduce:</p>
<pre><code>python benchmarks/telegram.py            # downloads the corpus on first run,
                                         # then 16 cold runs, ~5 min
python benchmarks/telegram_charts.py    # regenerates this page</code></pre>

</body>
</html>
"""
    OUT.write_text(html)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
