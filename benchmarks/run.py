"""Run the benchmark suite (rurex vs re) and write results.json.

Usage:
    python benchmarks/run.py [--quick] [--test NAME]

For every test and every sweep point, times the operation for both
engines end to end (auto-calibrated iterations like timeit, median of
3 repeats). The pathological test is single-shot with a 2 s timeout
guard on `re`.

Must be run against a release build: `maturin develop --release`.
"""

import json
import os
import pathlib
import platform
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import timeit

import re as stdre

import rurex
from suite import TESTS

HERE = pathlib.Path(__file__).parent
REPEATS = 3
PATHO_TIMEOUT_S = 2.0


def time_per_call(op, quick=False):
    timer = timeit.Timer(op)
    number, _ = timer.autorange()
    repeats = 2 if quick else REPEATS
    times = [t / number for t in timer.repeat(repeat=repeats, number=number)]
    return statistics.median(times)


def threaded_pile_point(pattern, text, nthreads, pile):
    """One threading benchmark point: a fixed pile of `pile` searches
    split across `nthreads` barrier-started workers; returns the
    wall-clock seconds to finish the whole pile (max of per-thread
    walls, which are equal by construction). One measurement per
    point — each is already an aggregate of thousands of ops."""
    import re as stdre

    import rurex

    rx = rurex.compile(pattern)
    rre = stdre.compile(pattern)
    iters = max(1, pile // nthreads)
    barrier = threading.Barrier(nthreads)

    def run_engine(engine):
        walls = [None] * nthreads

        def work(i):
            barrier.wait()
            t0 = time.perf_counter()
            for _ in range(iters):
                engine.search(text)
            walls[i] = time.perf_counter() - t0

        threads = [threading.Thread(target=work, args=(i,)) for i in range(nthreads)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        return max(walls)

    return run_engine(rre), run_engine(rx)


def guarded_once(op, timeout_s):
    """Run op once, returning seconds or None on timeout."""

    def handler(signum, frame):
        raise TimeoutError

    old = signal.signal(signal.SIGALRM, handler)
    signal.alarm(int(timeout_s) + 1)
    try:
        t0 = time.perf_counter()
        op()
        return time.perf_counter() - t0
    except TimeoutError:
        return None
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, old)


def measure_point(op_name, pattern, text, quick, value=None, pile=8000,
                  threads=None):
    re_rx = stdre.compile(pattern)
    rurex_rx = rurex.compile(pattern)

    if op_name == "threaded_search":
        nthreads = threads if threads is not None else value
        return threaded_pile_point(pattern, text, nthreads, pile)
    if op_name == "search":
        op_re = lambda: re_rx.search(text)
        op_ru = lambda: rurex_rx.search(text)
    elif op_name == "findall":
        op_re = lambda: re_rx.findall(text)
        op_ru = lambda: rurex_rx.findall(text)
    elif op_name == "guarded_search":
        # re explodes: single shot under a timeout guard.
        re_s = guarded_once(lambda: re_rx.search(text), PATHO_TIMEOUT_S)
        ru_s = time_per_call(lambda: rurex_rx.search(text), quick)
        return re_s, ru_s
    else:
        raise ValueError(op_name)

    return time_per_call(op_re, quick), time_per_call(op_ru, quick)


def run_test(test, quick):
    result = {
        "name": test["name"],
        "title": test["title"],
        "description": test["description"],
        "pattern": test["pattern"],
        "data_example": test["data_example"],
        "op": test["op"],
        "sweeps": [],
    }
    for sweep in test["sweeps"]:
        points = []
        for value in sweep["values"]:
            text = sweep["make"](value)
            re_s, ru_s = measure_point(
                test["op"], test["pattern"], text, quick,
                value=value, pile=test.get("pile", 8000),
                threads=test.get("threads"),
            )
            points.append(
                {"value": value, "re_s": re_s, "rurex_s": ru_s}
            )
            re_txt = fmt(re_s) if re_s is not None else ">= 2s (timeout)"
            print(
                f"    {sweep['param']}={value:<8} re {re_txt:>10}   "
                f"rurex {fmt(ru_s):>10}"
            )
        # rurex may sweep a wider range for guarded tests
        extra_values = sweep.get("values_rurex")
        if extra_values:
            for value in extra_values:
                if value in sweep["values"]:
                    continue
                text = sweep["make"](value)
                _, ru_s = measure_point(test["op"], test["pattern"], text, quick)
                points.append({"value": value, "re_s": None, "rurex_s": ru_s})
                print(
                    f"    {sweep['param']}={value:<8} re {'-':>10}   "
                    f"rurex {fmt(ru_s):>10}"
                )
        points.sort(key=lambda p: p["value"])
        result["sweeps"].append(
            {
                "param": sweep["param"],
                "unit": sweep["unit"],
                "label": sweep["label"],
                "points": points,
            }
        )
    return result


def environment():
    cpu = ""
    try:
        with open("/proc/cpuinfo") as f:
            for line in f:
                if line.startswith("model name"):
                    cpu = line.split(":", 1)[1].strip()
                    break
    except OSError:
        cpu = platform.processor() or platform.machine()
    return {
        "cpu": cpu,
        "python": platform.python_implementation() + " " + platform.python_version(),
        "os": f"{platform.system()} {platform.release()}",
        "rurex": rurex.__version__,
        "build": "maturin develop --release (opt-level=3, LTO=fat)",
        "timeout_s": PATHO_TIMEOUT_S,
    }


def fmt(seconds):
    if seconds is None:
        return "-"
    if seconds < 1e-6:
        return f"{seconds * 1e9:.0f} ns"
    if seconds < 1e-3:
        return f"{seconds * 1e6:.1f} µs"
    if seconds < 1:
        return f"{seconds * 1e3:.2f} ms"
    return f"{seconds:.2f} s"


def free_python():
    """Interpreter for the free-threaded tests: env override first,
    then the conventional .venv-t."""
    cand = os.environ.get("RUREX_FREE_PYTHON")
    if cand and pathlib.Path(cand).exists():
        return cand
    cand = HERE.parent / ".venv-t" / "bin" / "python"
    if cand.exists():
        return str(cand)
    return None


def is_freethreaded():
    return not getattr(sys, "_is_gil_enabled", lambda: True)()


def main():
    quick = "--quick" in sys.argv
    only = None
    if "--test" in sys.argv:
        only = sys.argv[sys.argv.index("--test") + 1]

    # Child mode: run ONLY the free-threaded tests (we ARE the
    # free-threaded interpreter) and write them to the given path.
    if "--free-child" in sys.argv:
        out_path = pathlib.Path(sys.argv[sys.argv.index("--free-child") + 1])
        child = {
            "environment": {"free_python": platform.python_version()},
            "tests": [],
        }
        if not is_freethreaded():
            raise SystemExit("--free-child requires a free-threaded interpreter")
        for test in TESTS:
            if test.get("interpreter") != "freethreaded":
                continue
            if only and test["name"] != only:
                continue
            print(test["title"])
            child["tests"].append(run_test(test, quick))
            print()
        with open(out_path, "w") as f:
            json.dump(child, f, indent=2)
        print(f"wrote {out_path}")
        return

    results = {"environment": environment(), "tests": []}
    results["environment"]["cpus"] = os.cpu_count()
    print(f"environment: {results['environment']['cpu']}\n")

    # GIL tests run in-process.
    for test in TESTS:
        if test.get("interpreter") == "freethreaded":
            continue
        if only and test["name"] != only:
            continue
        print(test["title"])
        results["tests"].append(run_test(test, quick))
        print()

    # Free-threaded tests run in a free-threaded subprocess.
    free_tests = [
        t for t in TESTS if t.get("interpreter") == "freethreaded"
        and (only is None or t["name"] == only)
    ]
    if free_tests:
        fp = free_python()
        if fp is None:
            print(
                "skipping free-threaded tests: no free-threaded "
                "interpreter found. Prepare one with:\n"
                "  uv venv --python 3.14t .venv-t\n"
                "  VIRTUAL_ENV=.venv-t uv pip install maturin pytest\n"
                "  .venv-t/bin/maturin develop --release\n"
                "or point RUREX_FREE_PYTHON at a t-interpreter with "
                "rurex installed."
            )
        else:
            print(f"free-threaded interpreter: {fp}")
            with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
                child_path = tf.name
            cmd = [fp, str(HERE / "run.py"), "--free-child", child_path]
            if quick:
                cmd.append("--quick")
            if only:
                cmd += ["--test", only]
            subprocess.run(cmd, check=True)
            child = json.loads(pathlib.Path(child_path).read_text())
            pathlib.Path(child_path).unlink()
            results["tests"].extend(child["tests"])
            results["environment"]["free_python"] = child["environment"][
                "free_python"
            ]

    out = HERE / "results.json"
    with open(out, "w") as f:
        json.dump(results, f, indent=2)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
