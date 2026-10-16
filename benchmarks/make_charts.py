"""Generate the benchmark charts from results.json.

Usage (from anywhere; paths are script-relative):
    python benchmarks/make_charts.py

Writes PNGs to docs/img/ and prints the paths.
"""

import json
import pathlib

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = pathlib.Path(__file__).parent
IMG = HERE.parent / "docs" / "img"
RESULTS = HERE / "results.json"

COLOR_RE = "#4C72B0"
COLOR_RUREX = "#CE422B"
COLOR_GRID = "#d9d9d9"

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
        "grid.color": COLOR_GRID,
        "grid.linewidth": 0.6,
        "axes.axisbelow": True,
        "figure.facecolor": "white",
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


def fmt_speedup(x):
    if x >= 1:
        return f"×{x:.1f} faster"
    return f"×{1 / x:.1f} slower"


def grouped_bars(cases, title, subtitle, out_name):
    names = [c["name"] for c in cases]
    re_t = [c["re_s"] for c in cases]
    ru_t = [c["rurex_s"] for c in cases]

    y = range(len(cases))
    h = 0.36
    fig, ax = plt.subplots(figsize=(9.5, 0.9 + 0.62 * len(cases)))
    ax.barh([i + h / 2 for i in y], re_t, height=h, color=COLOR_RE,
            label="re (stdlib)", zorder=3)
    ax.barh([i - h / 2 for i in y], ru_t, height=h, color=COLOR_RUREX,
            label="rurex", zorder=3)
    ax.set_xscale("log")
    ax.set_yticks(list(y))
    ax.set_yticklabels([n for n in names])
    ax.invert_yaxis()
    ax.set_xlabel("time per operation (log scale)")
    ax.set_title(title, loc="left", fontsize=14, fontweight="bold", pad=14)
    ax.text(0, 1.02, subtitle, transform=ax.transAxes, fontsize=9.5,
            color="#666666")

    for i, c in enumerate(cases):
        ax.text(c["re_s"], i + h / 2, "  " + fmt_time(c["re_s"]),
                va="center", ha="left", fontsize=8.5, color=COLOR_RE)
        ax.text(c["rurex_s"], i - h / 2, "  " + fmt_time(c["rurex_s"]),
                va="center", ha="left", fontsize=8.5, color=COLOR_RUREX)
        verdict = fmt_speedup(c["speedup"])
        color = "#1a7f37" if c["speedup"] > 1 else "#b35900"
        ax.text(1.0, i - h / 2, verdict, transform=ax.get_yaxis_transform(),
                va="center", ha="left", fontsize=9, fontweight="bold",
                color=color, clip_on=False)
    ax.set_xlim(right=max(max(re_t), max(ru_t)) * 3)
    ax.legend(loc="lower right", frameon=True, framealpha=0.9)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def summary_chart(cases, out_name):
    ordered = sorted(cases, key=lambda c: c["speedup"])
    names = [c["name"] for c in ordered]
    speedups = [c["speedup"] for c in ordered]
    colors = ["#b35900" if s < 1 else "#1a7f37" for s in speedups]

    fig, ax = plt.subplots(figsize=(9.5, 0.75 + 0.5 * len(cases)))
    ax.barh(range(len(ordered)), speedups, color=colors, zorder=3)
    ax.set_xscale("log")
    ax.set_xticks([0.05, 0.1, 0.25, 0.5, 1, 2, 4, 8, 16])
    ax.set_yticks(range(len(ordered)))
    ax.set_yticklabels(names)
    ax.axvline(1.0, color="#555555", linewidth=1.2, zorder=4)
    ax.set_xlabel("speedup, re time / rurex time (log scale) — right of the line: rurex faster")
    ax.set_title("Where each engine wins", loc="left", fontsize=14,
                 fontweight="bold", pad=14)
    ax.text(1.0, len(ordered) - 0.2, " parity ", ha="center", va="bottom",
            fontsize=8, color="#555555",
            bbox=dict(facecolor="white", edgecolor="#555555", pad=1.5))
    for i, s in enumerate(speedups):
        ax.text(s, i, "  " + fmt_speedup(s), va="center", ha="left",
                fontsize=9, fontweight="bold")
    ax.set_xlim(left=min(speedups) / 2.5, right=max(speedups) * 3.5)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def pathological_chart(data, out_name):
    series = [(n, t) for n, t in data["re_series"] if t is not None]
    ns = [n for n, _ in series]
    ts = [t for _, t in series]
    rurex_s = data["rurex_s_28"]

    fig, ax = plt.subplots(figsize=(9.5, 5.2))
    ax.plot(ns, ts, marker="o", color=COLOR_RE, linewidth=2.2,
            markersize=6, label="re (backtracking)", zorder=3)
    ax.axhline(rurex_s, color=COLOR_RUREX, linewidth=2.2, zorder=3)
    ax.text(ns[0], rurex_s * 1.35,
            f"rurex (finite automata): {fmt_time(rurex_s)} at every size",
            color=COLOR_RUREX, fontsize=10, fontweight="bold")
    timed_out = [n for n, t in data["re_series"] if t is None]
    if timed_out:
        ax.annotate(
            f"n={timed_out[0]}: still running\nat the 2 s timeout",
            xy=(ns[-1], ts[-1]),
            xytext=(ns[-1] - 5.5, ts[-1] * 0.12),
            fontsize=9.5,
            arrowprops=dict(arrowstyle="->", color="#555555"),
            bbox=dict(facecolor="#fff3cd", edgecolor="#d4a72c"),
        )
    ax.set_yscale("log")
    ax.set_xlabel("input length n, pattern (x+x+)+y on \"x\"*n")
    ax.set_ylabel("time per search (log scale)")
    ax.set_title("Catastrophic backtracking", loc="left", fontsize=14,
                 fontweight="bold", pad=14)
    ax.text(0, 1.02, "re's time doubles with every 2 extra characters; "
            "rurex is constant-time per input byte",
            transform=ax.transAxes, fontsize=9.5, color="#666666")
    ax.legend(loc="center left", frameon=True, framealpha=0.9)
    fig.tight_layout()
    path = IMG / out_name
    fig.savefig(path, bbox_inches="tight")
    plt.close(fig)
    return path


def main():
    data = json.loads(RESULTS.read_text())
    IMG.mkdir(parents=True, exist_ok=True)
    cases = data["cases"]

    groups = [
        ("scan", "Large-haystack scans (10 MB)",
         "Literal and simple patterns over big inputs — the automata engine's home turf"),
        ("match-heavy", "Match-heavy workloads (1 MB)",
         "findall / finditer / sub with tens to hundreds of thousands of matches"),
        ("overhead", "Call overhead",
         "Tiny inputs where fixed per-call cost dominates"),
        ("unicode", "Unicode and bytes (1 MB)",
         "Multilingual text and bytes-mode scanning"),
        ("compile", "Compilation",
         "Compiling automata costs more than compiling backtracking bytecode — "
         "amortize by compiling once (or via the module cache)"),
    ]
    for regime, title, subtitle in groups:
        subset = [c for c in cases if c["regime"] == regime]
        if not subset:
            continue
        p = grouped_bars(subset, title, subtitle, f"bench_{regime}.png")
        print("wrote", p)

    print("wrote", summary_chart(cases, "bench_summary.png"))
    print("wrote", pathological_chart(data["pathological"], "bench_pathological.png"))


if __name__ == "__main__":
    main()
