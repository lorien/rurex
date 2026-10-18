## Report on task: Measurement hardening and clean `regexus` benchmark run

Ad-hoc owner request: make A/B and published benchmarks use CPU-pinned
warm-up, record the environment, and validate the results on a clean
remote machine before committing anything. The earlier flow of
committing just to transfer code was explicitly rejected: validation
comes first, the commit is the validated artifact.

### Done

**Measurement protocol (`benchmarks/measurement.py`).**

- Auto-detects physical cores from `lscpu`/sysfs and selects the first
  four, never SMT siblings. On `regexus` that is CPU IDs `0,2,4,6` for
  cores `0,1,2,3`.
- Pins the process to those cores. For GIL builds the workers share the
  process affinity, so the active thread does not bounce across cores;
  for free-threaded builds each worker is pinned to one core.
- Each worker runs a fixed 1 s CPU warm-up on a separate 4 MiB buffer.
  The buffer uses `hashlib.sha256`, which releases the GIL for large
  inputs, so `re` workers warm the cores too. The measured corpus is
  never touched by the warm-up.
- Records affinity, governor, load average before/after, and per-CPU
  frequency before, after warm-up, and after the point.
- `RUREX_WARMUP` overrides the duration; quick runs use 0.25 s.

**A/B and repeated-run tooling.**

- `benchmarks/ab.py`: paired baseline/candidate runner with randomized
  order, the same warm-up/affinity protocol, median paired ratio, and a
  bootstrap confidence interval. `--aa` calibrates the noise floor.
- `benchmarks/merge_runs.py`: merges repeated full runs into a
  per-point median and records `wall_runs`.
- Both real-world benchmarks now use the protocol. Published rows are
  the median of three complete runs.

**Remote validation on `regexus`.**

- SSH as `web` was not configured (no `/home/web/.ssh`); the existing
  root access was used to install the local public key for `web`.
- Synced the uncommitted working tree with rsync to
  `web@regexus:/web/benchmark/rurex`, including `.git`, excluding
  `.env`, venvs, target, var and partial result files. No commit was
  made for transport.
- Installed Rust via rustup plus rustfmt/clippy, `python3.13-venv`,
  `uv`, CPython 3.14t, `.venv` and `.venv-t`. `maturin develop` failed
  in the uv venv until `pip` was installed there.
- Host: 13th Gen Intel Core i5-13500, 20 logical CPUs / 10 physical
  cores, 62 GiB RAM, load near 0.3 idle. Governor was set to
  `performance` for the runs (about 4.7–4.8 GHz) and restored to
  `powersave` afterwards.
- `make check` passed: 45 Rust tests, 295 pytest tests, 3,484
  conformance tests. Free-threaded pytest: 295 passed.

**Noise floor and A/B.**

- Homepage A/A, 4 workers, str: median ratio -0.34%, 95% CI
  [-1.64%, +0.94%].
- Transcode-detach A/B, homepage 4 workers, str, 20 paired samples:
  -2.72%, CI [-3.47%, -2.15%] on GIL; -2.13%, CI [-2.89%, -1.46%] on
  free-threaded. The bytes control moved +1.9% with a CI including
  zero, consistent with noise.

**Published remote results** (median of three full runs):

| Telegram | `re` str | rurex str | `re` bytes | rurex bytes |
|---|---:|---:|---:|---:|
| GIL 1 | 0.187 | 0.251 | 0.192 | 0.123 |
| GIL 4 | 0.189 | 0.282 | 0.191 | 0.153 |
| free 1 | 0.190 | 0.281 | 0.204 | 0.163 |
| free 4 | 0.097 | 0.160 | 0.104 | 0.161 |

| Homepage | `re` str | rurex str | `re` bytes | rurex bytes |
|---|---:|---:|---:|---:|
| GIL 1 | 3.423 | 0.615 | 1.698 | 0.491 |
| GIL 4 | 3.456 | 0.256 | 1.719 | 0.220 |
| free 1 | 3.180 | 0.601 | 1.907 | 0.481 |
| free 4 | 0.848 | 0.249 | 0.509 | 0.213 |

The remote numbers changed the Telegram narrative: `re` wins short-text
`str` at every thread count (×1.3 GIL single-threaded, ×1.5 at four
workers), and wins bytes at four free-threaded workers. rurex still
wins Telegram bytes single-threaded (×0.6) and wins homepage on both
forms by a wide margin. The README and both generated pages now say
this; the previous laptop claim of Telegram `str` near-parity was
measurement noise.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` documents the warm-up/affinity/frequency
  protocol, `benchmarks/ab.py`, `merge_runs.py`, and median-of-3
  publication.
- [acted] Both generated benchmark pages now describe CPU placement,
  warm-up, repeats, and the raw environment fields.
- No ADR: this is benchmark infrastructure, not library design.

### Future-task notes

- [open] `make build-free` fails on a uv-created `.venv-t` without
  `pip`; install `pip` in the venv or teach the target to use
  `maturin develop --uv`.
- [open] The remote `/web/benchmark/rurex` workspace now has web SSH
  access, Rust, venvs, and cached corpora. Decide whether to keep it as
  the standard clean benchmark host.
- [open] Published numbers are now from `regexus`; laptop numbers are
  no longer comparable. Keep the machine recorded in the result
  environment.
- [open] The benchmark scripts still do one run per invocation; the
  median-of-3 flow is documented but manual (`merge_runs.py`). A
  `--repeat` flag would make it one command.

### Tooling/process

- Do not commit to move code to a benchmark host. rsync the working
  tree, validate, then commit and push.
- The remote `web` user had no `.ssh` directory; root access was needed
  once to install the key and apt packages.
- A uv venv does not include pip; maturin's `develop` needs either pip
  or `--uv`.
- CPU pinning alone is not enough for GIL workloads: pinning each
  worker to a different core made `re` worse because the GIL moved
  across cores. The final protocol pins the process and lets GIL
  workers share that affinity, while free-threaded workers get one core
  each.
