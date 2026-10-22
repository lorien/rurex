"""Frontend HTML benchmark: href/src extraction over real pages.

Protocol — one real-world run per measured point. A fresh interpreter
process loads the corpus and performs the task exactly once: one
`findall` per HTML document over the whole pile, split across N
barrier-started workers. There is no warm-up and no repeat scan.

The corpus is a tar.gz of 2,086 real frontend HTML documents, stored as
a GitHub release asset and sha256-verified. It is cached under
`var/bench-data/`; the first run also builds a length-prefixed binary
copy so every fresh point can load it quickly.

    python benchmarks/frontend_html.py                  # 256 MiB subset, 32 cold runs
    python benchmarks/frontend_html.py --full           # all ~480 MiB
    python benchmarks/frontend_html.py --quick          # 32 MiB subset
    python benchmarks/frontend_html.py --point ENGINE NTHREADS FORM OUT.json

Timing window (see `single_point`): starts the instant the post-warm-up
barrier releases, stops the instant the slice is finished; the reported
wall is the max across workers. Load and pattern compile are outside the
window, identically for both engines. The whole matrix holds
`/tmp/core4.lock` so another benchmark cannot use the same cores.
"""

import hashlib
import json
import os
import pathlib
import platform
import statistics
import struct
import subprocess
import sys
import tarfile
import tempfile
import threading
import time

import corelock
import measurement

HERE = pathlib.Path(__file__).parent
RESULTS = HERE / "frontend_html_results.json"
DATA_CACHE = HERE.parent / "var" / "bench-data" / "frontend-html-500mb-v1.tgz"
BIN_CACHE = HERE.parent / "var" / "bench-data" / "frontend-html-500mb-v1.bin"
META_CACHE = HERE.parent / "var" / "bench-data" / "frontend-html-500mb-v1.meta.json"

DATA_URL = (
    "https://github.com/lorien/rurex/releases/download/"
    "bench-data-html-v1/frontend-html-500mb-v1.tgz"
)
DATA_SHA256 = (
    "a38962acb0b21bb603e8c0c7f4dfa357146e7c798d4041477b52773c1f53a330"
)
MAGIC = b"RUREXHT1"

# The subset bounds keep the default run around 300 MB RSS on a laptop.
# --full loads the whole ~480 MiB corpus (roughly 500-650 MB RSS).
DEFAULT_BYTES = 256 << 20
QUICK_BYTES = 32 << 20

# One dense, real extraction: quoted and unquoted attribute values.
PATTERN = r"""(?i)\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"""

THREADS = [1, 2, 3, 4]
ENGINES = ("re", "rurex")
INPUTS = ("str", "bytes")


def limit_bytes():
    if os.environ.get("RUREX_HTML_FULL"):
        return None
    if os.environ.get("RUREX_HTML_QUICK"):
        return QUICK_BYTES
    return DEFAULT_BYTES


def resolve_data():
    """Locate the tar.gz: $RUREX_HTML_DATA override -> cached copy ->
    download from the release asset (sha256-verified)."""
    env = os.environ.get("RUREX_HTML_DATA")
    if env:
        path = pathlib.Path(env)
        verify_archive(path)
        return path
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
        if got != DATA_SHA256:
            tmp.unlink(missing_ok=True)
            raise SystemExit(f"corpus sha256 mismatch: {got} != {DATA_SHA256}")
        tmp.rename(DATA_CACHE)
        print(f"cached at {DATA_CACHE}", flush=True)
    verify_archive(DATA_CACHE)
    return DATA_CACHE


def verify_archive(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    got = h.hexdigest()
    if got != DATA_SHA256:
        raise SystemExit(
            f"corpus sha256 mismatch: {got} != {DATA_SHA256}; "
            f"delete {path} to re-download"
        )


def build_corpus(archive):
    """Convert the HTML-only tar.gz into a length-prefixed binary file.

    Only `.html` members are accepted: anything else means the archive
    was not cleaned the way the benchmark expects.
    """
    print(f"building {BIN_CACHE.name} from {archive.name} ...", flush=True)
    BIN_CACHE.parent.mkdir(parents=True, exist_ok=True)
    tmp = BIN_CACHE.with_suffix(".part")
    count = 0
    total = 0
    with tarfile.open(archive, "r|gz") as tar, open(tmp, "wb") as out:
        out.write(MAGIC)
        out.write(b"\0\0\0\0")  # document count, patched at the end
        for member in tar:
            if not member.isfile():
                continue
            if not member.name.endswith(".html"):
                raise SystemExit(f"unexpected non-HTML member: {member.name}")
            data = tar.extractfile(member).read()
            if not data:
                continue
            try:
                data.decode("utf-8")
            except UnicodeDecodeError as exc:
                raise SystemExit(f"{member.name} is not valid UTF-8: {exc}")
            out.write(struct.pack("<I", len(data)))
            out.write(data)
            count += 1
            total += len(data)
        out.seek(len(MAGIC))
        out.write(struct.pack("<I", count))
    tmp.rename(BIN_CACHE)
    bin_hash = hashlib.sha256()
    with open(BIN_CACHE, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            bin_hash.update(chunk)
    meta = {
        "archive": archive.name,
        "archive_sha256": DATA_SHA256,
        "bin_sha256": bin_hash.hexdigest(),
        "documents": count,
        "total_bytes": total,
    }
    META_CACHE.write_text(json.dumps(meta, indent=2))
    print(
        f"built {count:,} documents, {total / 1e6:.1f} MB "
        f"({bin_hash.hexdigest()[:12]})",
        flush=True,
    )
    return meta


def resolve_corpus():
    archive = resolve_data()
    if BIN_CACHE.exists() and META_CACHE.exists():
        meta = json.loads(META_CACHE.read_text())
        if (
            meta.get("archive_sha256") == DATA_SHA256
            and meta.get("bin_sha256")
            and BIN_CACHE.stat().st_size
            == 12 + meta["total_bytes"] + 4 * meta["documents"]
        ):
            return meta
    return build_corpus(archive)


def load_docs(form, limit):
    """Load the corpus in the requested form, stopping once `limit`
    UTF-8 bytes have been read (None = all documents)."""
    docs = []
    total = 0
    with open(BIN_CACHE, "rb") as f:
        if f.read(len(MAGIC)) != MAGIC:
            raise SystemExit(f"{BIN_CACHE} has a bad magic header; delete it")
        count = struct.unpack("<I", f.read(4))[0]
        for _ in range(count):
            n = struct.unpack("<I", f.read(4))[0]
            data = f.read(n)
            if len(data) != n:
                raise SystemExit(f"{BIN_CACHE} is truncated")
            docs.append(data if form == "bytes" else data.decode("utf-8"))
            total += n
            if limit is not None and total >= limit:
                break
    return docs


def make_rx(engine, form):
    pat = PATTERN if form == "str" else PATTERN.encode()
    if engine == "re":
        import re as stdre

        return stdre.compile(pat)
    import rurex

    return rurex.compile(pat)


def result_hash(rx, docs):
    """Stable hash of every match tuple, for re/rurex parity checks."""
    h = hashlib.sha256()
    for doc in docs:
        for groups in rx.findall(doc):
            for part in groups:
                if isinstance(part, bytes):
                    b = part
                else:
                    b = part.encode("utf-8")
                h.update(struct.pack("<I", len(b)))
                h.update(b)
    return h.hexdigest()


def corpus_stats():
    """One dedicated run over the (limited) corpus, with parity check."""
    resolve_corpus()
    limit = limit_bytes()
    docs = load_docs("str", limit)
    import re as stdre

    import rurex

    ref = stdre.compile(PATTERN)
    ours = rurex.compile(PATTERN)
    total_bytes = 0
    total_matches = 0
    docs_with = 0
    per_doc = []
    doc_bytes = []
    for s in docs:
        nbytes = len(s.encode("utf-8"))
        total_bytes += nbytes
        doc_bytes.append(nbytes)
        n = len(ref.findall(s))
        per_doc.append(n)
        total_matches += n
        if n:
            docs_with += 1
    ref_hash = result_hash(ref, docs)
    ours_hash = result_hash(ours, docs)
    if ref_hash != ours_hash:
        raise SystemExit("rurex/re match hash mismatch; benchmark aborted")
    return {
        "documents": len(docs),
        "total_bytes": total_bytes,
        "total_matches": total_matches,
        "documents_with_matches": docs_with,
        "median_matches": statistics.median(per_doc),
        "max_matches": max(per_doc),
        "median_document_bytes": statistics.median(doc_bytes),
        "mean_document_bytes": total_bytes / len(docs),
        "result_sha256": ref_hash,
        "full": limit is None,
        "limit_bytes": limit,
    }


def single_point(nthreads, engine, form):
    """One real-world run in THIS (fresh) process. Returns
    (wall seconds, total matches)."""
    resolve_corpus()
    limit = limit_bytes()
    docs = load_docs(form, limit)
    rx = make_rx(engine, form)
    iters = (len(docs) + nthreads - 1) // nthreads
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
        part = docs[i * iters:(i + 1) * iters]  # slice taken untimed
        start.wait()                              # all workers warm together
        if warm_s > 0:
            measurement.warm_up(warm_s, warm_buf)
        warm_freq[i] = measurement.cpu_freq_khz([cpus[i]])
        run.wait()                                # all workers released
        cpu0 = time.thread_time()
        t0 = time.perf_counter()  # === START ===
        c = 0
        for doc in part:
            c += len(rx.findall(doc))
        counts[i] = c
        walls[i] = time.perf_counter() - t0  # === STOP ===
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


def _child_main():
    """`--point ...` or `--stats --point ...`: run one point, exit."""
    i = sys.argv.index("--point")
    if "--stats" in sys.argv:
        result = corpus_stats()
        print("  stats done", flush=True)
    else:
        engine, nthreads, form, out = (
            sys.argv[i + 1],
            int(sys.argv[i + 2]),
            sys.argv[i + 3],
            sys.argv[i + 4],
        )
        min_eff = measurement.min_efficiency()
        max_load = measurement.max_load()
        rejected = []
        for attempt in range(measurement.retries() + 1):
            wall, matches, info = single_point(nthreads, engine, form)
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
        result = {
            "engine": engine,
            "threads": nthreads,
            "input": form,
            "wall_s": wall,
            "matches": matches,
            "measurement": info,
            "attempts": attempt + 1,
            "rejected": rejected,
            "min_efficiency": min_eff,
        }
        print(
            f"  {engine:5} {form:5} threads={nthreads} {wall:.2f}s "
            f"({matches:,} matches, cpus={info['affinity_cpus']}, "
            f"warmup={info['warmup_s']:.2f}s, eff={info['efficiency']:.2f})",
            flush=True,
        )
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


def free_extension_identity(interpreter):
    """Identity of the extension the free-threaded interpreter loads.

    The points that use it run in child processes, so the parent cannot
    ask its own `rurex`; the same helper runs in the child instead.
    """
    if not interpreter:
        return None
    probe = (
        "import json, sys;"
        "sys.path.insert(0, %r);"
        "import measurement;"
        "print(json.dumps(measurement.extension_identity()))"
        % str(pathlib.Path(__file__).resolve().parent)
    )
    out = subprocess.run(
        [interpreter, "-c", probe], capture_output=True, text=True
    )
    if out.returncode != 0:
        return {"error": (out.stderr or out.stdout).strip()[:400]}
    return json.loads(out.stdout.strip().splitlines()[-1])


def free_python():
    cand = os.environ.get("RUREX_FREE_PYTHON")
    if cand and pathlib.Path(cand).exists():
        return cand
    cand = HERE.parent / ".venv-t" / "bin" / "python"
    return str(cand) if cand.exists() else None


def main():
    quick = "--quick" in sys.argv
    full = "--full" in sys.argv
    env = dict(os.environ)
    if quick:
        env["RUREX_HTML_QUICK"] = "1"
        env.pop("RUREX_HTML_FULL", None)
    elif full:
        env["RUREX_HTML_FULL"] = "1"
        env.pop("RUREX_HTML_QUICK", None)
    if quick and measurement.WARMUP_ENV not in env:
        env[measurement.WARMUP_ENV] = "0.25"

    meta = resolve_corpus()
    pretty = "full" if full else ("quick" if quick else "default")
    print(
        f"corpus: {meta['documents']:,} documents, "
        f"{meta['total_bytes'] / 1e6:.0f} MB ({pretty} subset)",
        flush=True,
    )

    interpreters = [("gil", sys.executable)]
    fp = free_python()
    if fp:
        interpreters.append(("free", fp))
    else:
        print("no free-threaded interpreter found; skipping free")

    script = str(pathlib.Path(__file__).resolve())
    results = {
        "environment": {
            "measured_at": measurement.measured_at(),
            "cpu": interpreter_env(),
            "pattern": PATTERN,
            "threads": THREADS,
            "extension": measurement.extension_identity(),
            "extension_free": free_extension_identity(fp),
            "corpus": {
                "url": DATA_URL,
                "sha256": DATA_SHA256,
                "documents": meta["documents"],
                "bytes": meta["total_bytes"],
                "mode": pretty,
            },
            "protocol": "one cold real-world run per point "
                        "(fresh process: load -> CPU-pinned warm-up -> "
                        "single pass; load and frequency recorded)",
        },
        "points": [],
    }

    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
        stats_path = tf.name
    subprocess.run(
        [sys.executable, script, "--stats", "--point", "-", "0", "-", stats_path],
        check=True,
        env=env,
    )
    results["stats"] = json.loads(pathlib.Path(stats_path).read_text())
    pathlib.Path(stats_path).unlink()
    s = results["stats"]
    print(
        f"stats: {s['documents']:,} docs, {s['total_bytes'] / 1e6:.0f} MB, "
        f"{s['total_matches']:,} matches in "
        f"{s['documents_with_matches']:,} docs",
        flush=True,
    )

    for interp_name, exe in interpreters:
        for nthreads in THREADS:
            for engine in ENGINES:
                for form in INPUTS:
                    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
                        out = tf.name
                    subprocess.run(
                        [exe, script, "--point", engine, str(nthreads), form, out],
                        check=True,
                        env=env,
                    )
                    point = json.loads(pathlib.Path(out).read_text())
                    pathlib.Path(out).unlink()
                    point["interpreter"] = interp_name
                    results["points"].append(point)
                    print(
                        f"  [{interp_name}] threads={nthreads} {engine:5} "
                        f"{form:5} {point['wall_s']:.2f}s "
                        f"({point['wall_s'] * 1e6 / s['documents']:.2f} "
                        f"us/doc, {point['matches']:,} matches)",
                        flush=True,
                    )

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
