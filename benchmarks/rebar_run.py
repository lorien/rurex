"""Measure rurex with rebar and rebuild the comparison page.

Usage:
    python benchmarks/rebar_run.py [--rebar DIR] [--cpu N] [--no-html]

This is the supported way to run the rebar comparison: it takes the
core lease, pins to one core, runs the two measurement sets, refuses to
publish anything if a point failed, and only then calls
benchmarks/rebar_charts.py, which rewrites docs/rebar-benchmark.md and
docs/rebar-benchmark.html from the fresh CSVs.

The engine selection is fixed on purpose: rurex in its text and byte
modes, stdlib `re` and `rust/regex`. See benchmarks/rebar/README.md for
the rebar-side setup (runner, engines.toml, definition edits).
"""

import argparse
import hashlib
import json
import os
import pathlib
import platform
import re
import subprocess
import sys
from datetime import date

HERE = pathlib.Path(__file__).parent
RESULTS = HERE / "rebar" / "results"
ENGINES = r"^(python/re|python/rurex|python/rurex-bytes|rust/regex)$"
SETS = [
    ("curated.csv", r"^curated/"),
    ("dict.csv", r"^dictionary/search/english-15$"),
]

sys.path.insert(0, str(HERE))
import corelock  # noqa: E402


def cpu_model():
    try:
        for line in open("/proc/cpuinfo"):
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or platform.machine()


def governor():
    try:
        return pathlib.Path(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"
        ).read_text().strip()
    except OSError:
        return "unknown"


def rebar_version(rebar):
    out = subprocess.run(
        [str(rebar), "version"], capture_output=True, text=True
    )
    text = (out.stdout + out.stderr).strip()
    m = re.search(r"(\d+\.\d+\.\d+)", text)
    return m.group(1) if m else "unknown"


def measure(rebar, benchmarks, pattern, out_path, cpu, max_time, max_iters):
    cmd = [
        "taskset",
        "-c",
        str(cpu),
        str(rebar),
        "measure",
        "-d",
        str(benchmarks),
        "-e",
        ENGINES,
        "-f",
        pattern,
        "--max-time",
        max_time,
        "--max-iters",
        str(max_iters),
    ]
    print("running:", " ".join(cmd), flush=True)
    with open(out_path, "w") as f:
        subprocess.run(cmd, stdout=f, check=True)


def validate(path, engines_expected):
    """Return (rows, engines) or raise when a point failed."""
    rows = []
    with open(path) as f:
        header = f.readline()
        if not header.startswith("name,"):
            raise SystemExit(f"{path}: unexpected header")
        for line in f:
            if not line.strip():
                continue
            fields = line.rstrip("\n").split(",")
            # name,model,rebar_version,engine,engine_version,err,...
            if len(fields) < 6:
                raise SystemExit(f"{path}: malformed row: {line[:80]}")
            if fields[5]:
                raise SystemExit(
                    f"{path}: point failed: {fields[0]} {fields[3]} err={fields[5]}"
                )
            rows.append(fields)
    if not rows:
        raise SystemExit(f"{path}: no measurements")
    engines = {fields[3] for fields in rows}
    missing = set(engines_expected) - engines
    if missing:
        raise SystemExit(f"{path}: missing engines {sorted(missing)}")
    return rows, engines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--rebar",
        default=os.environ.get("RUREX_REBAR", "/web/benchmark/rebar"),
        help="rebar checkout with a built target/release/rebar",
    )
    ap.add_argument("--cpu", type=int, default=0)
    ap.add_argument("--max-time", default="3s")
    ap.add_argument("--max-iters", type=int, default=10)
    ap.add_argument("--no-html", action="store_true")
    args = ap.parse_args()

    root = pathlib.Path(args.rebar)
    rebar = root / "target" / "release" / "rebar"
    benchmarks = root / "benchmarks"
    if not rebar.exists():
        raise SystemExit(f"no rebar binary at {rebar}; run `cargo build --release`")

    RESULTS.mkdir(parents=True, exist_ok=True)
    expected = ["python/re", "python/rurex", "rust/regex"]

    with corelock.CoreLock(command=sys.argv):
        for name, pattern in SETS:
            measure(
                rebar,
                benchmarks,
                pattern,
                RESULTS / name,
                args.cpu,
                args.max_time,
                args.max_iters,
            )
        engines = set(expected)
        for name, _ in SETS:
            rows, seen = validate(RESULTS / name, expected)
            engines |= seen
            print(f"{name}: {len(rows)} points, engines {sorted(seen)}")

    meta = {
        "measured_at": date.today().isoformat(),
        "host": platform.node(),
        "cpu": cpu_model(),
        "governor": governor(),
        "lease_cpus": corelock.CoreLock().cpus,
        "pinned_cpu": args.cpu,
        "max_time": args.max_time,
        "max_iters": args.max_iters,
        "rebar": rebar_version(rebar),
        "rurex": subprocess.run(
            [sys.executable, "-c", "import rurex; print(rurex.__version__)"],
            capture_output=True,
            text=True,
        ).stdout.strip(),
        "engines": sorted(engines),
        "files": {
            name: {
                "sha256": hashlib.sha256((RESULTS / name).read_bytes()).hexdigest(),
                "lines": (RESULTS / name).read_text().count("\n"),
            }
            for name, _ in SETS
        },
    }
    (RESULTS / "run.json").write_text(json.dumps(meta, indent=2) + "\n")

    cmd = [sys.executable, str(HERE / "rebar_charts.py")]
    if args.no_html:
        cmd.append("--no-html")
    subprocess.run(cmd, check=True)


if __name__ == "__main__":
    main()
