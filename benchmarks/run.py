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
import pathlib
import platform
import signal
import statistics
import sys
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


def measure_point(op_name, pattern, text, quick):
    re_rx = stdre.compile(pattern)
    rurex_rx = rurex.compile(pattern)

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
            re_s, ru_s = measure_point(test["op"], test["pattern"], text, quick)
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


def main():
    quick = "--quick" in sys.argv
    only = None
    if "--test" in sys.argv:
        only = sys.argv[sys.argv.index("--test") + 1]

    results = {"environment": environment(), "tests": []}
    print(f"environment: {results['environment']['cpu']}\n")

    for test in TESTS:
        if only and test["name"] != only:
            continue
        print(test["title"])
        results["tests"].append(run_test(test, quick))
        print()

    out = HERE / "results.json"
    with open(out, "w") as f:
        json.dump(results, f, indent=2)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
