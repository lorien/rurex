## Report on task: Per-call cost on free-threaded Python

### Task (archived from plan.md)

## Per-call cost on free-threaded Python

Status: done

The Telegram benchmark's free-threaded `bytes` row does not scale
(0.18 s at one worker, 0.15-0.20 s at four, against `re`'s
0.20 -> 0.09 s), and the row's own page blamed the shared `regex` cache
pool. That is refuted: per-thread engine clones remove 5.3% of the
instructions at four workers and change neither cycles nor wall time,
while costing ~1.5% per call single threaded (ADR-0018 has the
measurements; the change was implemented, measured and reverted).

What the numbers do say, on 200k posts, per-post wall time of one
worker at 1 versus 4 workers: `len(t)` 1.15x, `t.count(b"@")` on the
same buffers 1.2x, `re.search` with the same pattern 1.9x,
`rurex.is_match` 2.6-3.6x, `rurex.findall` 3.5x, `rurex` `finditer`
count 4.2x. At four workers 51% of self time is CPython's shared
reference-count paths (`_Py_IncRef`, `_Py_DecRefShared`, `Py_DECREF`)
against 18% at one, while rurex's own frames are ~12% of the profile
(`memchr` 6.2%, engine pool 5.8%). So the loss is per-Python-call cost
that grows with the number of Python-visible operations per call, and
a call that creates no objects at all (`is_match`) already pays it.

The task is therefore: find and remove binding-side per-call Python
work, measured at four free-threaded workers with the guard active
(worker efficiency and physical-core pinning, as `telegram.py` does),
with single-thread cost as a hard constraint.

Order of investigation:

1. Instrument the trampoline itself: compare a `#[pyclass]` method call
   against a plain C function and a C method of the same signature
   (arguments, return value) at 1 and 4 workers, to split "PyO3
   trampoline" from "our body". The pool and haystack-refcount levers
   are already measured and dead.
2. Then attack whatever the split names, in this order of suspicion:
   argument extraction for `(string, pos = 0, endpos = None)`, the
   `Match` handle pair per match (`Py<Regex>` + `Py<PyAny>`), and the
   `Vec<Bound>` + `PyList::new` item path in grouped `findall`.
3. If the split shows the cost is PyO3's fastcall-with-keywords
   trampoline rather than the body, the fix is a plain positional
   signature (no keyword defaults) or a hand-written `METH_FASTCALL`
   entry, and it needs owner sign-off on the API surface it would
   touch.

Rejected up front: more engine-side isolation (per-thread pools or
caches) — measured in ADR-0018 — and caching across calls (a per-thread
last-haystack cache would change semantics for mutable buffers).

References: ADR-0018, ADR-0017,
`spec/report/2026/10/18/report-1792361198-contamination-guard.md`,
`spec/report/2026/10/20/report-1792570216-capture-locations.md`

### Done

- **Split the trampoline from the body** with a throwaway PyO3 probe
  extension (`/tmp/probe-ext`, not committed): the same trivial body
  exposed as a staticmethod, as an instance method on a mutable class,
  as an instance method on a frozen class with rurex's exact
  `(string, pos = 0, endpos = None)` signature, and with an extra
  `usize` argument.
- **Found that the previous session's refutation was a measurement
  artefact.** The multi-worker probes used then created a fresh set of
  worker threads for every round and every case, so a run accumulated
  hundreds of thread states and the four-worker numbers (2.6–3.6×
  inflation, 272–400 ns per call) reflected that churn. With one case
  per process and threads created once, the numbers fall to 1.0–2.8×
  and the pool hypothesis holds: the same engine costs 134 ns per
  `is_match` at four workers when shared, 89 ns when cloned per thread.
- **Shipped the per-thread engine clones** (`src/regex.rs`): the
  ADR-0017 thread-local table now holds an engine clone next to the
  `CaptureLocations` scratch, keyed by the pattern id; two ids are
  reserved per pattern so the lazily built anchored `fullmatch` engine
  gets its own slot; `is_match`, `search`/`match`/`fullmatch`,
  `finditer`/scanner steps, `sub`, `split` and the flat `findall` scan
  all drive the thread's clone.
- Verified behaviour under eight threads and across more patterns than
  the table has slots, then ran the full suite and all conformance
  layers.
- Republished the Telegram page from the median of three fresh runs, and
  its free-threaded commentary now says what the measurements say
  (result objects, not the scan).

### Results

Controlled A/B on the benchmark host, three alternating rounds per
wheel, threads created once per process, core lease held. Per-post wall
time of one worker over 200k Telegram posts:

| workload | 1 worker (before → after) | 4 workers (before → after) |
|---|---|---|
| `is_match` | 94 → 96 ns | **136–152 → 99–101 ns** |
| `findall` | 164–166 → 165–167 ns | 311–346 → 300–308 ns |
| `finditer` count | 240–245 → 243–244 ns | 683–708 → 661–687 ns |

`is_match` goes from 1.5× inflation at four workers to 1.04×; the two
result-building workloads gain 4% and 3%, because their per-call cost is
the Python objects they create. Telegram-shaped `findall` over the same
posts: 248 → 205 ns per worker at two workers, 331 → 302 at four,
unchanged at one.

Probe split (the trampoline itself, 200k posts, per-post wall):

| call | 1 worker | 4 workers |
|---|---:|---:|
| `len(t)` | 39 ns | 32 ns |
| `t.count(b"@")` | 122 ns | 118 ns |
| PyO3 staticmethod | 49 ns | 47 ns |
| PyO3 frozen method, rurex's signature | 47 ns | 40 ns |
| PyO3 mutable-class method, same signature | 53 ns | 135 ns |
| that method plus an extracted `usize` | 69 ns | 246 ns |

So rurex's signature shape and its frozen classes are not a problem; an
extracted integer argument is (rurex's `pos`/`endpos` defaults are
Rust-side, so nothing is extracted unless the caller passes them), and
a mutable pyclass pays 2.5× at four workers.

The published Telegram page is unchanged within its noise (median of
three cold runs: bytes 0.18 → 0.20 s at four workers), which is
consistent with the 9% controlled gain on a row dominated by result
objects.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0019-per-thread-engines.md`: the probe
  split, the corrected pool measurement, the decision (clone per thread,
  two ids per pattern, every matching site, bounded storage) and the
  rejected alternatives (`search_with` + explicit `Cache`, one engine
  per thread total, pointer keying, "advise one pattern per thread").
- [acted] `spec/docs/adr/0018-free-threaded-per-call-cost.md` is kept
  and marked superseded, with the reason (its probe recreated threads
  per round, invalidating every four-worker number).
- [acted] `spec/docs/architecture.md` "Matching path" now describes the
  per-thread engine and why the shared pool was the blocker.
- [acted] `spec/docs/index.md` catalogs ADR-0019.
- [acted] `spec/docs/plan.md`: this record is archived; a new record
  ("Python result objects on free-threaded Python") carries the
  remaining measurements and the probe recipe.
- [acted] `docs/telegram-benchmark.md`/`.html` and its charts are
  republished from three merged runs, and the free-threaded commentary
  now attributes the residual cost to result objects with the measured
  inflation factors.

### Future-task notes

- [open] The new plan record's targets, in measured order: the `Match`
  handle pair for `finditer` (2.8× at four workers), grouped `findall`'s
  `Vec<Bound>` plus list, and callable `sub`/`split`.
- [open] A mutable pyclass costs 2.5× at four workers on the borrow
  flag; every rurex class is frozen, so nothing to do — but any future
  class that is not frozen will inherit that cost, and the probe shows
  how to check it.
- [open] An extracted integer argument costs 3.6× at four workers. If
  an API ever takes a per-call integer (a `count=` on `finditer`, say),
  measure it first.
- [open] Engine clones mean each thread builds its own DFA/hybrid
  caches, bounded by the engine's limits. A workload with many patterns
  per thread and four threads should be checked for memory (four
  patterns × threads × caches).

### Tooling/process

- **The probe shape decided the answer, twice.** Worker threads must be
  created once per measured point (not per round, not per case), and one
  case should own the process: with threads recreated per round the
  four-worker numbers came out 2–3× too high and reversed the
  conclusion. The recipe is now written into the plan record.
- Per-worker pinning (`measurement.pin_current(cpus[i])`) plus
  `pick_physical_cpus` is what makes the four-worker points meaningful;
  wrapping the whole probe in `taskset -c 0` silently pins every worker
  to one core and produces a fake "no scaling" result.
- Hoisting the callable and the data slice into locals keeps the probe
  from measuring `LOAD_GLOBAL`/shared-refcount contention in its own
  loop.
- Keeping the probe as a separate throwaway crate (`/tmp/probe-ext`,
  maturin + pyo3 + regex) avoided adding benchmark-only code to the
  binding, and let the same-body/different-signature comparison be a
  one-line change.
- `perf` on this host needs `-e task-clock`: the default event binds to
  one of the hybrid PMUs and records almost nothing.
