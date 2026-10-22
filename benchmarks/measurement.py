"""Shared measurement hardening for the real-world benchmarks.

Every measured point runs the same protocol:

- the process is pinned to distinct physical CPUs; free-threaded
  workers are pinned one per core, while GIL workers share the process
  affinity so the active thread does not bounce across cores;
- all workers run a fixed CPU-bound warm-up on a separate buffer, so the
  cores are at a high frequency before the timed window;
- the governor, load average and per-CPU frequency are recorded around
  the warm-up and the measured pass.

The warm-up buffer is never the measured corpus, so the measured pass is
still a cold pass over the real data.
"""

import hashlib
import os
import pathlib
import platform
import importlib.machinery
import statistics
import subprocess
import sys
import time

import rurex
from datetime import datetime, timezone

WARMUP_ENV = "RUREX_WARMUP"
DEFAULT_WARMUP_S = 1.0
WARMUP_BUFFER_BYTES = 4 << 20


def _lscpu_topology():
    """Map (socket, core) to the list of logical CPUs, via lscpu."""
    try:
        out = subprocess.check_output(
            ["lscpu", "-p=CPU,CORE,SOCKET"],
            text=True,
            stderr=subprocess.DEVNULL,
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError):
        return {}
    cores = {}
    for line in out.splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split(",")
        if len(parts) < 2:
            continue
        try:
            cpu = int(parts[0])
            core = int(parts[1])
            socket = int(parts[2]) if len(parts) > 2 else 0
        except ValueError:
            continue
        cores.setdefault((socket, core), []).append(cpu)
    return cores


def _sysfs_topology():
    """Fallback: read thread_siblings_list for every online CPU."""
    root = pathlib.Path("/sys/devices/system/cpu")
    cores = {}
    for cpu_dir in sorted(root.glob("cpu[0-9]*")):
        try:
            cpu = int(cpu_dir.name[3:])
        except ValueError:
            continue
        siblings = cpu_dir / "topology" / "thread_siblings_list"
        try:
            text = siblings.read_text().strip()
        except OSError:
            text = str(cpu)
        first = text.split(",")[0]
        if "-" in first:
            first = first.split("-")[0]
        try:
            core = int(first)
        except ValueError:
            core = cpu
        cores.setdefault((0, core), []).append(cpu)
    return cores


def _available_cpus():
    try:
        return set(os.sched_getaffinity(0))
    except (AttributeError, OSError):
        return set(range(os.cpu_count() or 1))


def pick_physical_cpus(nthreads):
    """Choose nthreads CPUs, preferring one distinct physical core each.

    Returns (cpus, distinct) where cpus has exactly nthreads entries. When
    the machine has fewer physical cores than workers, the remaining
    workers reuse cores and `distinct` is False.
    """
    available = _available_cpus()
    cores = _lscpu_topology() or _sysfs_topology()
    chosen = []
    if cores:
        for cpus in sorted(cores.values(), key=lambda c: min(c)):
            usable = sorted(cpu for cpu in cpus if cpu in available)
            if usable:
                chosen.append(usable[0])
    if not chosen:
        chosen = sorted(available)
    if not chosen:
        chosen = [0]
    distinct = len(chosen) >= nthreads
    used = [chosen[i % len(chosen)] for i in range(nthreads)]
    return used, distinct


def pin_process(cpus):
    """Pin the current process to the given CPUs. Returns True on success."""
    try:
        os.sched_setaffinity(0, set(cpus))
        return True
    except (AttributeError, OSError):
        return False


def pin_current(cpu):
    """Pin the calling thread to one CPU. Returns True on success."""
    try:
        os.sched_setaffinity(0, {cpu})
        return True
    except (AttributeError, OSError):
        return False


def _read_int(path):
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return None


def cpu_freq_khz(cpus):
    """Current scaling frequency for each requested CPU, where available."""
    vals = []
    for cpu in cpus:
        path = pathlib.Path(
            f"/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq"
        )
        val = _read_int(path)
        if val is not None:
            vals.append(val)
    return vals


def governor(cpus):
    vals = set()
    for cpu in cpus:
        path = pathlib.Path(
            f"/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_governor"
        )
        try:
            vals.add(path.read_text().strip())
        except OSError:
            pass
    return sorted(vals)


def loadavg():
    try:
        text = pathlib.Path("/proc/loadavg").read_text().split()
        return [float(x) for x in text[:3]]
    except (OSError, ValueError):
        return None


def environment(cpus):
    return {
        "affinity_cpus": list(cpus),
        "governor": governor(cpus),
        "loadavg": loadavg(),
        "freq_khz": cpu_freq_khz(cpus),
    }


def warmup_seconds():
    raw = os.environ.get(WARMUP_ENV)
    if raw is None:
        return DEFAULT_WARMUP_S
    try:
        return max(0.0, float(raw))
    except ValueError:
        return DEFAULT_WARMUP_S


def make_warmup_buffer(size=WARMUP_BUFFER_BYTES):
    # A deterministic buffer is enough; it is only used to keep the cores
    # busy. It is never mixed with the measured corpus.
    return bytes(size)


def warm_up(seconds, buffer):
    """Keep the calling core busy until the deadline.

    hashlib releases the GIL for large buffers, so this warms every pinned
    core even for the `re` engine, which would otherwise hold the GIL.
    """
    deadline = time.perf_counter() + seconds
    while time.perf_counter() < deadline:
        hashlib.sha256(buffer).digest()


def gil_enabled():
    try:
        return bool(sys._is_gil_enabled())
    except AttributeError:
        return True


def warmup_summary(seconds, distinct, cpus, pinned):
    return {
        "warmup_s": seconds,
        "distinct_physical_cores": distinct,
        "affinity_cpus": list(cpus),
        "affinity_pinned": pinned,
    }


def median_or_none(values):
    return statistics.median(values) if values else None


def min_efficiency():
    raw = os.environ.get("RUREX_MIN_EFFICIENCY")
    if raw is None:
        return 0.90
    try:
        return max(0.0, min(1.0, float(raw)))
    except ValueError:
        return 0.90


def retries():
    raw = os.environ.get("RUREX_RETRIES")
    if raw is None:
        return 2
    try:
        return max(0, int(raw))
    except ValueError:
        return 2


def max_load():
    raw = os.environ.get("RUREX_MAX_LOAD")
    if raw is None or raw == "":
        return None
    try:
        return float(raw)
    except ValueError:
        return None


def extension_identity():
    """Identify the extension binary this interpreter is actually running.

    `results.json` used to carry a hard-coded "built with --release"
    string, which cannot prove which artifact was measured: a debug or
    stale extension produced the same text. Record what the process
    loaded instead — path, size, mtime and sha256 — so a published
    number is traceable to one binary and two runs are comparable. The
    compiler profile still comes from `make build-release` (and
    `build-free`); the hash only proves *which* artifact those produced.
    """
    info = {
        "python": platform.python_version(),
        "executable": sys.executable,
        "gil_enabled": bool(getattr(sys, "_is_gil_enabled", lambda: True)()),
    }
    pkg = pathlib.Path(rurex.__file__).resolve().parent
    suffixes = importlib.machinery.EXTENSION_SUFFIXES
    ext = next(
        (
            p
            for p in sorted(pkg.iterdir())
            if any(p.name.endswith(s) for s in suffixes)
        ),
        None,
    )
    if ext is None:
        info["path"] = None
        return info
    digest = hashlib.sha256()
    with open(ext, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    st = ext.stat()
    info.update(
        {
            "path": str(ext),
            "size": st.st_size,
            "mtime_ns": st.st_mtime_ns,
            "sha256": digest.hexdigest(),
        }
    )
    return info


def measured_at():
    """UTC timestamp of a run, for the result file and the page footer."""
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def efficiency(thread_cpu, walls, gil=None):
    """CPU efficiency for one measured point.

    GIL builds run one worker at a time, so use total thread CPU over
    wall. Free-threaded builds pin one worker per core, so use the
    minimum worker CPU over that worker's wall.
    """
    if not walls or max(walls) <= 0:
        return 0.0, "none"
    if gil is None:
        gil = gil_enabled()
    process = sum(thread_cpu) / max(walls)
    per = [c / w if w > 0 else 0.0 for c, w in zip(thread_cpu, walls)]
    per_worker = min(per) if per else 0.0
    if not gil:
        return per_worker, "per-worker"
    # GIL builds normally run one worker at a time, but rurex releases the
    # GIL for large haystacks. A process ratio well above 1 proves real
    # parallelism, so use the per-worker floor there; otherwise a worker
    # waiting on the GIL would look like a stolen core.
    if process > 1.15:
        return per_worker, "per-worker"
    return process, "process"


def quality_ok(info, min_eff, max_load_value):
    """Return (ok, reason) for a measured point."""
    if info.get("efficiency", 0.0) < min_eff:
        return False, (
            f"efficiency {info.get('efficiency', 0.0):.3f} < {min_eff:.3f}"
        )
    loads = []
    for key in ("loadavg_before", "loadavg_after"):
        load = info.get(key)
        if load:
            loads.append(load[0])
    if max_load_value is not None and loads and max(loads) > max_load_value:
        return False, f"load {max(loads):.2f} > {max_load_value:.2f}"
    return True, ""
