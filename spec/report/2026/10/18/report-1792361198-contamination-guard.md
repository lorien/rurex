## Report on task: Benchmark contamination guard

Ad-hoc owner request: the core lease only stops cooperative processes,
so add per-worker CPU-efficiency checks, reject contaminated points,
and retry before publishing.

### Done

- Added measurement helpers:
  - `efficiency(thread_cpu, walls)`:
    - GIL builds use process CPU / wall, because only one worker runs at
      a time; a worker waiting on the GIL must not look like a stolen
      core.
    - If a GIL point's process ratio is above 1.15, the engine really is
      running in parallel (rurex releases the GIL on large haystacks),
      so the per-worker minimum is used.
    - Free-threaded builds always use the minimum worker CPU / wall.
  - `quality_ok(info, min_eff, max_load)` rejects low efficiency or high
    load.
  - `RUREX_MIN_EFFICIENCY` (default 0.90), `RUREX_RETRIES` (default 2),
    and `RUREX_MAX_LOAD` (disabled by default) environment controls.
- Telegram and homepage runners now record worker CPU and wall time,
  efficiency and efficiency kind, and retry a rejected point up to the
  configured limit. Exhausted retries abort instead of writing a result.
- `ab.py` accepts `--min-efficiency`, `--retries`, and `--max-load` and
  passes them to the paired child points.
- `run.py` guards threaded points; GIL runs use process efficiency
  because per-worker efficiency produced a false rejection when rurex
  released the GIL only for part of the call. Free-threaded runs use
  per-worker efficiency.
- `merge_runs.py` recomputes efficiency from the recorded worker CPU and
  wall arrays when merging repeated runs.
- Docs updated in `spec/docs/dev.md`, `docs/benchmarks.md`, and both
  generated benchmark pages.
- Added `tests/test_measurement.py` for efficiency modes, rejection
  reasons, and environment parsing.

### Validation

- Local: `make check` passes (45 Rust, 309 pytest, 3,484 conformance);
  free-threaded pytest passes.
- Remote `regexus`: sync of the uncommitted tree, 14 corelock and
  measurement tests pass. A guarded homepage point reports efficiency
  0.979 per-worker; a guarded Telegram point reports 1.104 process.
- Three guarded full runs of each benchmark on `regexus` completed with
  zero rejected points. The merged results now record `attempts: 1` and
  efficiency values between about 0.98 and 1.12; the published timing
  rows are unchanged within noise.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` documents the guard, defaults, and retries.
- [acted] Telegram and homepage methodology pages include the
  contamination guard.
- No ADR: benchmark infrastructure only.

### Future-task notes

- [open] The 0.90 efficiency floor and 1.15 GIL-parallel threshold are
  heuristics; tune them if a benchmark row starts retrying on an idle
  machine.
- [open] `RUREX_MAX_LOAD` is disabled by default so laptop runs with
  ordinary background load are not rejected automatically; set it
  explicitly on a controlled host.
- [open] The shared regex cache-pool contention behind the flat Telegram
  bytes scaling is still unaddressed; the explicit per-thread cache
  remains the next optimization candidate.

### Tooling/process

- The first `run.py --test threads_gil` attempt after adding per-worker
  checks was falsely rejected at efficiency 0.78 because rurex releases
  the GIL only during part of a call. The GIL path now uses process
  efficiency, while free-threaded points still require every worker to
  stay above the floor.
- The remote runs showed the guard accepts clean points with efficiency
  near 1.0, so the default threshold is not too strict on an isolated
  host.
