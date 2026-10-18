"""Merge repeated full benchmark runs into one median result file.

    python benchmarks/merge_runs.py run1.json run2.json run3.json \
        -o benchmarks/telegram_results.json

Every input must be a result file from the same benchmark, with the same
corpus stats and point keys. The output keeps the first run's
environment and stats, replaces each wall time with the median across
runs, and records the individual walls in `wall_runs`.
"""

import argparse
import json
import pathlib
import statistics


def key(point):
    return (
        point["interpreter"],
        point["threads"],
        point["engine"],
        point["input"],
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("runs", nargs="+")
    ap.add_argument("-o", "--output", required=True)
    args = ap.parse_args()
    runs = [json.loads(pathlib.Path(p).read_text()) for p in args.runs]
    if not runs:
        raise SystemExit("no runs")
    base = runs[0]
    base_stats = base.get("stats")
    maps = []
    for run in runs:
        if run.get("stats") != base_stats:
            raise SystemExit("runs have different corpus statistics")
        m = {key(p): p for p in run["points"]}
        if not maps:
            maps.append(m)
        else:
            if set(m) != set(maps[0]):
                raise SystemExit("runs have different point sets")
            maps.append(m)
    points = []
    for k, first in maps[0].items():
        p = dict(first)
        walls = [m[k]["wall_s"] for m in maps]
        p["wall_s"] = statistics.median(walls)
        p["wall_runs"] = walls
        points.append(p)
    base["points"] = points
    protocol = base.get("environment", {}).get("protocol", "")
    suffix = f"median of {len(runs)} runs"
    if suffix not in protocol:
        base["environment"]["protocol"] = (
            protocol + f" ({suffix})" if protocol else suffix
        )
    pathlib.Path(args.output).write_text(json.dumps(base, indent=2))
    print(f"wrote {args.output} ({len(runs)} runs, {len(points)} points)")


if __name__ == "__main__":
    main()
