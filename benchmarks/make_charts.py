"""Generate the benchmark charts (line charts: parameter vs time) and
the rendered HTML twin of the results page.

Usage:
    python benchmarks/make_charts.py            # charts only
    python benchmarks/make_charts.py --html     # charts + docs/benchmarks.html

Charts go to docs/img/, named bench_<test>_<param>.png. The HTML
render embeds images as base64 data URIs so the file opens standalone
in any browser.
"""

import base64
import json
import pathlib
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import markdown  # pip install markdown

HERE = pathlib.Path(__file__).parent
DOCS = HERE.parent / "docs"
IMG = DOCS / "img"
RESULTS = HERE / "results.json"

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


def fmt_time(seconds):
    if seconds < 1e-6:
        return f"{seconds * 1e9:.0f} ns"
    if seconds < 1e-3:
        return f"{seconds * 1e6:.1f} µs"
    if seconds < 1:
        return f"{seconds * 1e3:.1f} ms"
    return f"{seconds:.2f} s"


def sweep_chart(test, sweep, out_name, logx=False, logy=True):
    xs_re = [p["value"] for p in sweep["points"] if p["re_s"] is not None]
    ys_re = [p["re_s"] for p in sweep["points"] if p["re_s"] is not None]
    xs_ru = [p["value"] for p in sweep["points"]]
    ys_ru = [p["rurex_s"] for p in sweep["points"]]

    fig, ax = plt.subplots(figsize=(9.5, 5.4))
    ax.plot(xs_re, ys_re, marker="o", color=COLOR_RE, linewidth=2.2,
            markersize=6, label="re (stdlib)", zorder=3)
    ax.plot(xs_ru, ys_ru, marker="o", color=COLOR_RUREX, linewidth=2.2,
            markersize=6, label="rurex", zorder=3)

    timed_out = [p for p in sweep["points"] if p["re_s"] is None]
    if timed_out and xs_re:
        last = [p for p in sweep["points"] if p["re_s"] is not None][-1]
        beyond = timed_out[0]
        ax.plot(
            [last["value"], beyond["value"]],
            [last["re_s"], last["re_s"] * 4],
            color=COLOR_RE, linewidth=2.0, linestyle=(0, (4, 3)), zorder=3,
        )
        ax.annotate(
            f"n={beyond['value']}: still running\nat the 2 s timeout",
            xy=(beyond["value"], last["re_s"] * 4),
            xytext=(beyond["value"] * 0.45, last["re_s"] * 0.02),
            fontsize=9.5,
            arrowprops=dict(arrowstyle="->", color="#555555"),
            bbox=dict(facecolor="#fff3cd", edgecolor="#d4a72c"),
        )

    # Annotate the final measured point of each series.
    if xs_re:
        ax.text(xs_re[-1], ys_re[-1], "  " + fmt_time(ys_re[-1]),
                color=COLOR_RE, fontsize=9, va="bottom", ha="left")
    ax.text(xs_ru[-1], ys_ru[-1], "  " + fmt_time(ys_ru[-1]),
            color=COLOR_RUREX, fontsize=9, va="top", ha="left")

    if logx:
        ax.set_xscale("log")
    if logy:
        ax.set_yscale("log")
    ax.set_xlabel(f"{sweep['param']} ({sweep['unit']})")
    ax.set_ylabel("time per operation" + (" (log scale)" if logy else ""))
    ax.set_title(f"{test['title'].split('—')[0].strip()} — {sweep['param']}",
                 loc="left", fontsize=13, fontweight="bold", pad=12)
    ax.text(0, 1.02, sweep["label"], transform=ax.transAxes, fontsize=9.5,
            color="#666666")
    ax.legend(loc="best")
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


# Per-sweep presentation hints: (test, param) -> dict(logx=, logy=)
PRESENTATION = {
    ("scan", "data_size"): dict(logx=True, logy=True),
    ("scan", "match_position"): dict(logx=False, logy=True),
    ("extract", "data_size"): dict(logx=True, logy=True),
    ("extract", "match_density"): dict(logx=False, logy=False),
    ("pathological", "input_length"): dict(logx=False, logy=True),
}


def generate_charts(data):
    IMG.mkdir(parents=True, exist_ok=True)
    written = []
    for test in data["tests"]:
        for sweep in test["sweeps"]:
            hints = PRESENTATION.get((test["name"], sweep["param"]), {})
            name = f"bench_{test['name']}_{sweep['param']}.png"
            written.append(sweep_chart(test, sweep, name, **hints))
    return written


HTML_TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>rurex — benchmarks</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
         max-width: 880px; margin: 2rem auto; padding: 0 1rem; color: #1f2328; line-height: 1.6; }}
  code {{ background: #f6f8fa; padding: .2em .4em; border-radius: 6px; font-size: 85%;
         font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
  pre {{ background: #f6f8fa; padding: 14px 16px; border-radius: 8px; overflow-x: auto; }}
  pre code {{ background: none; padding: 0; font-size: 85%; }}
  h1, h2 {{ border-bottom: 1px solid #d1d9e0; padding-bottom: .3em; }}
  table {{ border-collapse: collapse; margin: 1rem 0; }}
  th, td {{ border: 1px solid #d1d9e0; padding: 6px 13px; text-align: left; }}
  th {{ background: #f6f8fa; }}
  td.num, th.num {{ text-align: right; }}
  img {{ max-width: 100%; }}
  blockquote {{ border-left: 4px solid #0969da; padding: .4rem 1rem;
                background: #f6f8fa; border-radius: 0 6px 6px 0; }}
  .banner {{ background: #fff8c5; border: 1px solid #d4a72c; border-radius: 8px;
             padding: .6rem 1rem; margin-bottom: 2rem; font-size: 90%; }}
</style>
</head>
<body>
<div class="banner">Rendered from <code>docs/benchmarks.md</code> with embedded
images — regenerate with <code>python benchmarks/make_charts.py --html</code>.</div>
{body}
</body>
</html>
"""


def render_html():
    md_text = (DOCS / "benchmarks.md").read_text()
    body = markdown.markdown(md_text, extensions=["tables", "fenced_code"])
    # Embed local images as data URIs.
    import re

    def embed(match):
        src = match.group(1)
        path = DOCS / src
        b64 = base64.b64encode(path.read_bytes()).decode()
        return f'src="data:image/png;base64,{b64}"'

    body = re.sub(r'src="(img/[^"]+)"', embed, body)
    out = DOCS / "benchmarks.html"
    out.write_text(HTML_TEMPLATE.format(body=body))
    return out


def main():
    data = json.loads(RESULTS.read_text())
    for p in generate_charts(data):
        print("wrote", p)
    if "--html" in sys.argv:
        print("wrote", render_html())


if __name__ == "__main__":
    main()
