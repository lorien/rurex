"""Run the rurex-vs-re benchmark suite.

Usage:
    python benchmarks/run.py [--quick]

Times every case for both engines end to end (what a Python user
experiences, including object construction), auto-calibrating the
iteration count like timeit and taking the median of 5 repeats.
Writes benchmarks/results.json and prints a result table.

Must be run against a release build: `maturin develop --release`.
"""

import json
import platform
import signal
import statistics
import sys
import time
import timeit

import re as stdre

import rurex
from cases import CASES, COMPILE_CASES, DISPATCH_CASE, PATHOLOGICAL

REPEATS = 5


def time_per_call(op, quick=False):
    timer = timeit.Timer(op)
    number, _ = timer.autorange()
    if quick:
        return min(timer.repeat(repeat=2, number=number)) / number
    times = [t / number for t in timer.repeat(repeat=REPEATS, number=number)]
    return statistics.median(times)


def run_case(case):
    pattern = case["pattern"]
    text = case["text"]
    method = case["method"]
    is_bytes = case.get("mode") == "bytes"
    re_rx = stdre.compile(pattern)
    rurex_rx = rurex.compile(pattern)

    if method == "finditer":
        op_re = lambda: [m.span() for m in re_rx.finditer(text)]
        op_rurex = lambda: [m.span() for m in rurex_rx.finditer(text)]
    elif method == "sub":
        repl = case["repl"]
        if is_bytes:
            repl = repl.encode()
        op_re = lambda: re_rx.sub(repl, text)
        op_rurex = lambda: rurex_rx.sub(repl, text)
    elif method == "is_match":
        # re has no is_match; the equivalent user-level check.
        op_re = lambda: re_rx.search(text) is not None
        op_rurex = lambda: rurex_rx.is_match(text)
    else:
        op_re = (lambda f=getattr(re_rx, method), t=text: f(t))
        op_rurex = (lambda f=getattr(rurex_rx, method), t=text: f(t))

    re_s = time_per_call(op_re)
    rurex_s = time_per_call(op_rurex)
    return re_s, rurex_s


def run_compile(case):
    pattern = case["pattern"]

    def re_compile_uncached():
        # re.compile() itself is backed by an internal pattern cache;
        # purge it so we measure a real compilation, like rurex does
        # (rurex.compile is uncached by design).
        stdre.purge()
        stdre.compile(pattern)

    return time_per_call(re_compile_uncached), time_per_call(
        lambda: rurex.compile(pattern)
    )


def run_dispatch(case):
    pattern, text = case["pattern"], case["text"]
    return time_per_call(lambda: stdre.search(pattern, text)), time_per_call(
        lambda: rurex.search(pattern, text)
    )


def run_pathological(timeout_s=2.0):
    """re's time grows exponentially with input length; cap it."""
    pattern = PATHOLOGICAL["pattern"]

    def guarded(text):
        def handler(signum, frame):
            raise TimeoutError

        old = signal.signal(signal.SIGALRM, handler)
        signal.alarm(int(timeout_s) + 1)
        try:
            t0 = time.perf_counter()
            stdre.search(pattern, text)
            return time.perf_counter() - t0
        except TimeoutError:
            return None
        finally:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, old)

    re_series = []
    n = 8
    while n <= 28:
        text = "x" * n
        t = guarded(text)
        re_series.append((n, t))
        if t is None or t > timeout_s:
            break
        n += 2

    rx = rurex.compile(pattern)
    big = "x" * 28
    rurex_s = time_per_call(lambda: rx.search(big))
    return re_series, rurex_s


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
        "stdre": "re (" + stdre.__name__ + ") from stdlib",
        "build": "maturin develop --release (opt-level=3, LTO=fat)",
    }


def main():
    quick = "--quick" in sys.argv
    results = {"environment": environment(), "cases": [], "pathological": {}}

    print(f"environment: {results['environment']['cpu']}")
    print()

    all_cases = []
    for case in CASES:
        all_cases.append(("case", case))
    for case in COMPILE_CASES:
        all_cases.append(("compile", case))
    all_cases.append(("dispatch", DISPATCH_CASE))

    for kind, case in all_cases:
        if kind == "case":
            re_s, ru_s = run_case(case)
        elif kind == "compile":
            re_s, ru_s = run_compile(case)
        else:
            re_s, ru_s = run_dispatch(case)
        speedup = re_s / ru_s
        entry = dict(case)
        entry.pop("text", None)
        if isinstance(entry.get("pattern"), bytes):
            entry["pattern"] = repr(entry["pattern"])
        entry.update({"re_s": re_s, "rurex_s": ru_s, "speedup": speedup})
        results["cases"].append(entry)
        verdict = "rurex faster" if speedup > 1 else "re faster"
        print(
            f"{case['name']:<28} re {fmt(re_s):>10}   rurex {fmt(ru_s):>10}   "
            f"x{speedup:.2f} {verdict}"
        )

    print("\npathological (x+x+)+y ...")
    re_series, rurex_s = run_pathological()
    results["pathological"] = {
        "pattern": PATHOLOGICAL["pattern"],
        "re_series": re_series,
        "rurex_s_28": rurex_s,
        "timeout_s": 2.0,
    }
    for n, t in re_series:
        print(f"  re  n={n:>2}: {fmt(t) if t is not None else '>= 2s (timeout)'}")
    print(f"  rurex n=28: {fmt(rurex_s)}")

    import pathlib
    out = pathlib.Path(__file__).parent / "results.json"
    with open(out, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nwrote {out}")


def fmt(seconds):
    if seconds is None:
        return "-"
    if seconds < 1e-6:
        return f"{seconds * 1e9:.1f} ns"
    if seconds < 1e-3:
        return f"{seconds * 1e6:.1f} us"
    if seconds < 1:
        return f"{seconds * 1e3:.2f} ms"
    return f"{seconds:.2f} s"


if __name__ == "__main__":
    main()
