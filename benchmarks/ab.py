"""Paired A/B runner for the real-world benchmarks.

Runs baseline and candidate extensions in separate fresh processes with
the benchmark's CPU-pinned warm-up protocol, alternating the order and
reporting the paired ratio distribution. Use --aa to calibrate the
measurement noise floor with the same protocol.

    python benchmarks/ab.py --bench homepage --threads 1,4 \
        --forms str,bytes --samples 6 \
        --baseline var/baseline-so313 --candidate var/candidate-so313

    python benchmarks/ab.py --bench homepage --aa --samples 10

Each side directory must contain an importable rurex extension (for
example `rurex.cpython-313-x86_64-linux-gnu.so`). With no directory the
currently installed rurex is used. The whole paired matrix holds
`/tmp/core4.lock`; child processes inherit the lease marker.
"""

import argparse
import json
import os
import pathlib
import random
import statistics
import subprocess
import sys
import tempfile

import corelock

HERE = pathlib.Path(__file__).parent
SCRIPTS = {
    "telegram": HERE / "telegram.py",
    "homepage": HERE / "frontend_html.py",
}


def parse_csv(text, cast=str):
    return [cast(x.strip()) for x in text.split(",") if x.strip()]


def py_path(directory, env):
    if not directory:
        return env
    old = env.get("PYTHONPATH")
    env["PYTHONPATH"] = directory + (os.pathsep + old if old else "")
    return env


def run_point(python, bench, nthreads, form, directory, warmup):
    env = dict(os.environ)
    env["RUREX_WARMUP"] = str(warmup)
    env = py_path(directory, env)
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
        out = tf.name
    try:
        subprocess.run(
            [
                python,
                str(SCRIPTS[bench]),
                "--point",
                "rurex",
                str(nthreads),
                form,
                out,
            ],
            check=True,
            env=env,
            stdout=subprocess.DEVNULL,
        )
        data = json.loads(pathlib.Path(out).read_text())
    finally:
        pathlib.Path(out).unlink(missing_ok=True)
    return data


def bootstrap_ci(samples, rounds=2000, seed=1234):
    if len(samples) < 2:
        return (None, None)
    rng = random.Random(seed)
    meds = []
    for _ in range(rounds):
        pick = [samples[rng.randrange(len(samples))] for _ in samples]
        meds.append(statistics.median(pick))
    meds.sort()
    return meds[int(0.025 * rounds)], meds[int(0.975 * rounds)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bench", choices=sorted(SCRIPTS), default="homepage")
    ap.add_argument("--python", default=str(HERE.parent / ".venv/bin/python"))
    ap.add_argument("--threads", default="1,4")
    ap.add_argument("--forms", default="str,bytes")
    ap.add_argument("--samples", type=int, default=6)
    ap.add_argument("--warmup", type=float, default=1.0)
    ap.add_argument("--baseline", default="")
    ap.add_argument("--candidate", default="")
    ap.add_argument("--aa", action="store_true",
                    help="run the same extension on both sides")
    ap.add_argument("--json", default="")
    ap.add_argument("--seed", type=int, default=1234)
    args = ap.parse_args()

    if args.aa:
        args.candidate = args.baseline
    rng = random.Random(args.seed)
    results = []
    for nthreads in parse_csv(args.threads, int):
        for form in parse_csv(args.forms):
            base = []
            cand = []
            for _ in range(args.samples):
                order = ["baseline", "candidate"]
                rng.shuffle(order)
                for side in order:
                    directory = (
                        args.baseline if side == "baseline" else args.candidate
                    )
                    point = run_point(
                        args.python,
                        args.bench,
                        nthreads,
                        form,
                        directory,
                        args.warmup,
                    )
                    (base if side == "baseline" else cand).append(
                        point["wall_s"]
                    )
            ratios = [(c - b) / b for b, c in zip(base, cand)]
            med = statistics.median(ratios)
            lo, hi = bootstrap_ci(ratios)
            row = {
                "threads": nthreads,
                "input": form,
                "baseline": base,
                "candidate": cand,
                "ratio_median": med,
                "ratio_mean": statistics.mean(ratios),
                "ratio_ci95": [lo, hi],
                "baseline_median": statistics.median(base),
                "candidate_median": statistics.median(cand),
            }
            results.append(row)
            print(
                f"threads={nthreads} {form:5} "
                f"base={row['baseline_median']:.3f}s "
                f"cand={row['candidate_median']:.3f}s "
                f"ratio={med:+.4f} "
                f"ci95=[{lo:+.4f},{hi:+.4f}]"
                if lo is not None
                else f"threads={nthreads} {form} ratio={med:+.4f}",
                flush=True,
            )

    payload = {
        "bench": args.bench,
        "python": args.python,
        "baseline": args.baseline,
        "candidate": args.candidate,
        "aa": args.aa,
        "samples": args.samples,
        "warmup_s": args.warmup,
        "results": results,
    }
    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(payload, indent=2))
        print(f"wrote {args.json}")


if __name__ == "__main__":
    with corelock.CoreLock(command=sys.argv):
        os.environ[corelock.HELD_ENV] = "1"
        main()
