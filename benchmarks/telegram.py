"""Telegram real-world benchmark: @username extraction over 1M posts.

Protocol — one real-world run per measured point. A fresh interpreter
process loads the corpus and performs the task exactly once: one
`findall` per post over the whole pile, split across N barrier-started
workers. There is no warm-up and no repeat scan: every point is cold,
including CPython's first-touch UTF-8 materialization of each string
(PyUnicode_AsUTF8AndSize caches the UTF-8 form inside every str
object on first use) — the price a user scanning the corpus once
actually pays.

Timing window (see `single_point`): starts the instant a worker
passes the barrier, stops the instant its slice is finished; the
reported wall is the max across workers. Load and pattern compile are
outside the window, identically for both engines.

    python benchmarks/telegram.py                  # 32 cold runs, writes telegram_results.json
    python benchmarks/telegram.py --quick          # first 100k posts per run
    python benchmarks/telegram.py --point RE NTHREADS OUT.json   # internal child
"""

import gzip
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import sys
import tempfile
import threading
import time

import corelock
import measurement

HERE = pathlib.Path(__file__).parent
RESULTS = HERE / "telegram_results.json"

# The corpus lives outside git (93 MB gzip / 527 MB raw) as a GitHub
# release asset; resolve_data() downloads it once into the gitignored
# var/bench-data/ cache and sha256-verifies it. $RUREX_TG_DATA
# overrides the location (a .jsonl or .gz file).
DATA_URL = (
    "https://github.com/lorien/rurex/releases/download/"
    "bench-data-telegram-v1/tg-1m.json.gz"
)
DATA_GZ_SHA256 = (
    "eb91db735fcf1a9e6096a59e959471cb9a3c4f41438f864e800cdb66f994023b"
)
DATA_CACHE = HERE.parent / "var" / "bench-data" / "tg-1m.json.gz"

# Telegram username rules: 5-32 chars, [a-zA-Z0-9_], starts with a
# letter.
PATTERN = r"@[a-zA-Z][a-zA-Z0-9_]{4,31}"
THREADS = [1, 2, 3, 4]
ENGINES = ("re", "rurex")
INPUTS = ("str", "bytes")


def resolve_data() -> pathlib.Path:
    """Locate the corpus: $RUREX_TG_DATA override -> cached copy ->
    download from the release asset (sha256-verified)."""
    env = os.environ.get("RUREX_TG_DATA")
    if env:
        return pathlib.Path(env)
    if not DATA_CACHE.exists():
        import urllib.request

        print(f"downloading corpus ({DATA_URL}) ...", flush=True)
        DATA_CACHE.parent.mkdir(parents=True, exist_ok=True)
        tmp = DATA_CACHE.with_suffix(".part")
        h = hashlib.sha256()
        with urllib.request.urlopen(DATA_URL) as r, open(tmp, "wb") as f:
            while True:
                chunk = r.read(1 << 20)
                if not chunk:
                    break
                h.update(chunk)
                f.write(chunk)
        got = h.hexdigest()
        if got != DATA_GZ_SHA256:
            tmp.unlink(missing_ok=True)
            raise SystemExit(
                f"corpus sha256 mismatch: {got} != {DATA_GZ_SHA256}"
            )
        tmp.rename(DATA_CACHE)
        print(f"cached at {DATA_CACHE}", flush=True)
    h = hashlib.sha256(DATA_CACHE.read_bytes())
    if h.hexdigest() != DATA_GZ_SHA256:
        raise SystemExit(
            f"cached corpus sha256 mismatch: {DATA_CACHE}; delete it "
            f"to re-download"
        )
    return DATA_CACHE


def load_texts(path, limit=None):
    texts = []
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        for line in f:
            texts.append(json.loads(line)["text"])
            if limit and len(texts) >= limit:
                break
    return texts


def make_rx(engine, form):
    pat = PATTERN if form == "str" else PATTERN.encode()
    if engine == "re":
        import re as stdre

        return stdre.compile(pat)
    import rurex

    return rurex.compile(pat)


def single_point(nthreads, engine, form):
    """One real-world run in THIS (fresh) process. Returns
    (wall seconds, total mentions).

    Untimed setup: load+parse the corpus, prepare the input form
    (the bytes variant encodes every post to UTF-8 HERE, never
    inside the timed loop), and compile the matching pattern form —
    one compile, shared by all workers. Timed window: from the
    barrier release to the worker's slice end, one findall per post.

    The str variant keeps CPython's first-touch UTF-8 materialization
    inside the window (it is triggered by the findall call itself);
    the bytes variant measures pure scan+extraction on prepared
    UTF-8.
    """
    limit = int(os.environ.get("RUREX_TG_LIMIT", "0")) or None
    texts = load_texts(resolve_data(), limit)
    if form == "bytes":
        texts = [t.encode("utf-8") for t in texts]
    rx = make_rx(engine, form)
    iters = (len(texts) + nthreads - 1) // nthreads
    cpus, distinct = measurement.pick_physical_cpus(nthreads)
    pinned = measurement.pin_process(cpus)
    env_before = measurement.environment(cpus)
    warm_s = measurement.warmup_seconds()
    warm_buf = measurement.make_warmup_buffer() if warm_s > 0 else b""
    start = threading.Barrier(nthreads)
    run = threading.Barrier(nthreads)
    walls = [0.0] * nthreads
    counts = [0] * nthreads
    thread_cpu = [0.0] * nthreads
    warm_freq = [None] * nthreads
    worker_pinned = [False] * nthreads
    pin_workers = not measurement.gil_enabled()

    def work(i):
        worker_pinned[i] = (
            measurement.pin_current(cpus[i]) if pin_workers else pinned
        )
        part = texts[i * iters:(i + 1) * iters]  # slice taken untimed
        start.wait()                              # all workers warm together
        if warm_s > 0:
            measurement.warm_up(warm_s, warm_buf)
        warm_freq[i] = measurement.cpu_freq_khz([cpus[i]])
        run.wait()                                # all workers released
        cpu0 = time.thread_time()
        t0 = time.perf_counter()                  # === START ===
        c = 0
        for t in part:                            # one pass, each post
            c += len(rx.findall(t))               # exactly once
        counts[i] = c
        walls[i] = time.perf_counter() - t0       # === STOP ===
        thread_cpu[i] = time.thread_time() - cpu0

    threads = [threading.Thread(target=work, args=(i,)) for i in range(nthreads)]
    for th in threads:
        th.start()
    for th in threads:
        th.join()
    env_after = measurement.environment(cpus)
    eff, eff_kind = measurement.efficiency(thread_cpu, walls)
    info = measurement.warmup_summary(warm_s, distinct, cpus, pinned)
    info.update({
        "worker_affinity_pinned": worker_pinned,
        "worker_pinning": "per-core" if pin_workers else "process",
        "worker_wall_s": walls,
        "worker_cpu_s": thread_cpu,
        "efficiency": eff,
        "efficiency_kind": eff_kind,
        "frequency_before_khz": env_before["freq_khz"],
        "frequency_after_warmup_khz": [
            vals[0] if vals else None for vals in warm_freq
        ],
        "frequency_after_khz": env_after["freq_khz"],
        "governor": env_before["governor"],
        "loadavg_before": env_before["loadavg"],
        "loadavg_after": env_after["loadavg"],
    })
    return max(walls), sum(counts), info


def corpus_stats():
    """One dedicated run (stdlib re) over the (limited) corpus."""
    limit = int(os.environ.get("RUREX_TG_LIMIT", "0")) or None
    texts = load_texts(resolve_data(), limit)
    import re as stdre

    rx = stdre.compile(PATTERN)
    posts_with = 0
    total = 0
    bytes_ = 0
    for t in texts:
        bytes_ += len(t.encode("utf-8"))
        n = len(rx.findall(t))
        total += n
        if n:
            posts_with += 1
    return {"posts": len(texts), "total_bytes": bytes_,
            "total_mentions": total, "posts_with_mentions": posts_with}


def _child_main():
    """`--point ENGINE NTHREADS OUT.json`: run one point, exit."""
    i = sys.argv.index("--point")
    if "--stats" in sys.argv:
        result = corpus_stats()
        print("  stats done", flush=True)
    else:
        engine, nthreads, form, out = (sys.argv[i + 1], int(sys.argv[i + 2]),
                                       sys.argv[i + 3], sys.argv[i + 4])
        min_eff = measurement.min_efficiency()
        max_load = measurement.max_load()
        rejected = []
        for attempt in range(measurement.retries() + 1):
            wall, mentions, info = single_point(nthreads, engine, form)
            ok, reason = measurement.quality_ok(info, min_eff, max_load)
            if ok:
                break
            rejected.append({"reason": reason,
                             "efficiency": info["efficiency"],
                             "loadavg": info.get("loadavg_before")})
            print(f"  rejected {engine} {form} threads={nthreads}: {reason}",
                  flush=True)
        else:
            raise SystemExit(
                f"benchmark point rejected after {measurement.retries() + 1} "
                f"attempts: {rejected[-1]['reason']}"
            )
        result = {"engine": engine, "threads": nthreads, "input": form,
                  "wall_s": wall, "mentions": mentions, "measurement": info,
                  "attempts": attempt + 1, "rejected": rejected,
                  "min_efficiency": min_eff}
        print(f"  {engine:5} {form:5} threads={nthreads} {wall:.2f}s "
              f"({mentions:,} mentions, cpus={info['affinity_cpus']}, "
              f"warmup={info['warmup_s']:.2f}s, eff={info['efficiency']:.2f})",
              flush=True)
    pathlib.Path(sys.argv[i + 4]).write_text(json.dumps(result))


def child_main():
    if os.environ.get(corelock.HELD_ENV):
        _child_main()
        return
    with corelock.CoreLock(command=sys.argv):
        os.environ[corelock.HELD_ENV] = "1"
        _child_main()


def interpreter_env():
    cpu = ""
    try:
        with open("/proc/cpuinfo") as f:
            for line in f:
                if line.startswith("model name"):
                    cpu = line.split(":", 1)[1].strip()
                    break
    except OSError:
        cpu = platform.processor() or platform.machine()
    return cpu


def free_python():
    cand = os.environ.get("RUREX_FREE_PYTHON")
    if cand and pathlib.Path(cand).exists():
        return cand
    cand = HERE.parent / ".venv-t" / "bin" / "python"
    return str(cand) if cand.exists() else None


def main():
    quick = "--quick" in sys.argv
    env = dict(os.environ)
    if quick:
        env["RUREX_TG_LIMIT"] = "100000"
    if quick and measurement.WARMUP_ENV not in env:
        env[measurement.WARMUP_ENV] = "0.25"

    interpreters = [("gil", sys.executable)]
    fp = free_python()
    if fp:
        interpreters.append(("free", fp))
    else:
        print("no free-threaded interpreter found; skipping free")

    script = str(pathlib.Path(__file__).resolve())
    results = {
        "environment": {
            "cpu": interpreter_env(),
            "pattern": PATTERN,
            "threads": THREADS,
            "protocol": "one cold real-world run per point "
                        "(fresh process: load -> CPU-pinned warm-up -> "
                        "single pass; load and frequency recorded)",
        },
        "points": [],
    }

    # Corpus stats once (stdlib re).
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
        stats_path = tf.name
    subprocess.run([sys.executable, script, "--stats", "--point",
                    "-", "0", "-", stats_path], check=True, env=env)
    results["stats"] = json.loads(pathlib.Path(stats_path).read_text())
    pathlib.Path(stats_path).unlink()
    s = results["stats"]
    print(f"corpus: {s['posts']:,} posts, {s['total_bytes'] / 1e6:.0f} MB, "
          f"{s['total_mentions']:,} mentions in {s['posts_with_mentions']:,} posts")

    for interp_name, exe in interpreters:
        for nthreads in THREADS:
            for engine in ENGINES:
                for form in INPUTS:
                    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
                        out = tf.name
                    subprocess.run(
                        [exe, script, "--point", engine, str(nthreads), form, out],
                        check=True, env=env,
                    )
                    point = json.loads(pathlib.Path(out).read_text())
                    pathlib.Path(out).unlink()
                    point["interpreter"] = interp_name
                    results["points"].append(point)
                    print(f"  [{interp_name}] threads={nthreads} {engine:5} "
                          f"{form:5} {point['wall_s']:.2f}s "
                          f"({point['wall_s'] * 1e6 / s['posts']:.2f} µs/post, "
                          f"{point['mentions']:,} mentions)", flush=True)

    with open(RESULTS, "w") as f:
        json.dump(results, f, indent=2)
    print(f"wrote {RESULTS}")


if __name__ == "__main__":
    if "--point" in sys.argv:
        child_main()
    else:
        with corelock.CoreLock(command=sys.argv):
            os.environ[corelock.HELD_ENV] = "1"
            main()
