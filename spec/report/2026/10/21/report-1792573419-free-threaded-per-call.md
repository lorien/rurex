## Report on task: Per-thread engines to remove free-threaded cache-pool contention

Owner instruction: "adapt task as you see it fits". The task was
investigated in full, its premise was refuted by measurement, the code
change was reverted, and the record is re-scoped to the measured cause.
No record is archived: the task stays in `plan.md` under a new title.

### Done

- **Implemented the documented remedy.** A per-thread table keyed by the
  ADR-0017 pattern id, extended to hold a clone of the engine (`Regex`
  for `str`, `bytes::Regex` for `bytes`) next to the `CaptureLocations`
  scratch; a second reserved id per pattern for the lazily built
  anchored `fullmatch` engine, so a clone of that engine can never be
  handed to the main one; `is_match`, `search`/`match`/`fullmatch`,
  `finditer` and scanner steps, `sub`, `split` and the flat `findall`
  scan all driving the thread's clone. `regex-automata`'s `Clone`
  deliberately drops the `Arc` around the pool, so each clone does get
  its own cache pool — verified in its source before relying on it.
- Suite green with it: `cargo test` 45, pytest 439, CPython
  `test_re.py`, the rust-regex corpus and the divergence corpus, plus an
  eight-thread correctness check.
- **Measured it on the benchmark host** (core lease, alternating wheels,
  `perf` now installed there): 5.3% fewer instructions at four workers,
  no change in cycles or wall time, ~1.5% slower per call single
  threaded. Reverted.
- **Found what the inflation actually is.** Per-post wall cost at one
  versus four free-threaded workers, same 200k posts: `len(t)` 1.15×,
  `t.count(b"@")` over the same buffers 1.2×, `re.search` with the same
  pattern and data 1.9×, `rurex.is_match` 2.6–3.6×, `rurex.findall`
  3.5×, `rurex` `finditer` count 4.2×. `perf` self time at four workers
  is 51% CPython's shared reference-count paths (`_Py_IncRef` 21.5%,
  `_Py_DecRefShared` 17.8%, `Py_DECREF` 11.9%) against 18% at one, while
  rurex's own frames are ~12% of the four-worker profile (`memchr` 6.2%,
  engine pool 5.8%).
- **Tried the other binding-side lever and rejected it too**: making
  `HayKeep` borrow the haystack instead of cloning its `Bound` (one
  INCREF/DECREF pair per call on a foreign-thread-owned object) measured
  111 → 104–106 ns per `is_match` on the free-threaded build, but
  100 → 109 ns on the GIL build for a group-less pattern, and nothing at
  four workers. Not shipped; the numbers are in ADR-0018.
- Re-scoped the plan record ("Per-call cost on free-threaded Python")
  with the decomposition and an ordered investigation, and recorded the
  refutation in ADR-0018 so the next session starts from the
  measurements.

### Results

Telegram corpus, `findall`, four free-threaded workers, three
alternating runs:

| | before | with per-thread engines |
|---|---:|---:|
| wall | 0.0289 s | 0.0296 s |
| cpu_core cycles | 22.54e9 | 22.34e9 |
| instructions | 22.43e9 | 21.25e9 (−5.3%) |

Inflation at four workers relative to one, per post:

| operation | 1 worker | 4 workers | inflation |
|---|---:|---:|---:|
| `len(t)` | 34 ns | 39–40 ns | 1.15× |
| `t.count(b"@")` | 124 ns | 146–148 ns | 1.2× |
| `re.search`, same pattern | 208 ns | 389–392 ns | 1.9× |
| `rurex.is_match` | 104–111 ns | 272–400 ns | 2.6–3.6× |
| `rurex.findall` | 178 ns | 623–639 ns | 3.5× |
| `rurex` `finditer` count | 253 ns | 1061–1134 ns | 4.2× |

Four-worker `perf` self time: `_Py_IncRef` 21.5%, `_Py_DecRefShared`
17.8%, `Py_DECREF` 11.9% (51% total) against 18% at one worker;
`rurex`/`regex_automata` frames `memchr` 6.2%, pool 5.8%, meta
`is_match` 1.9%, hybrid 1.4%.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0018-free-threaded-per-call-cost.md`: the
  pool hypothesis, the clones-as-remedy experiment and its numbers, the
  per-call decomposition, the decision not to isolate engines per
  thread, and the rejected alternatives (per-thread `Cache` via
  `search_with`, the unconditional haystack borrow, "just document it").
- [acted] `spec/docs/plan.md`: the task record is rewritten as
  "Per-call cost on free-threaded Python" — same priority, the measured
  cause, and an ordered investigation starting with the trampoline
  split.
- [acted] `spec/docs/architecture.md` "Matching path" now states what
  dominates free-threaded per-call cost and that engine isolation is
  not the lever.
- [acted] `spec/docs/index.md` catalogs ADR-0018.
- [acted] The contamination-guard report's `[open]` item ("the shared
  regex cache-pool contention … remains the next optimization
  candidate") is marked `[acted]` with the outcome.

### Future-task notes

- [acted] The trampoline split is the first step of the new record: a
  `#[pyclass]` method versus a plain C function versus a C method of
  the same signature, at one and four workers. Until that is measured,
  no further binding-side per-call change should be attempted.
  [acted — measured with a throwaway probe crate; see ADR-0019 and
  report-1792576652-per-thread-engines. The split also exposed that this
  report's own multi-worker numbers were probe artefacts.]
- [acted] If the split names PyO3's fastcall-with-keywords trampoline,
  the candidate fixes are a positional-only signature (dropping the
  `pos`/`endpos` defaults is an API change and needs owner sign-off) or
  a hand-written `METH_FASTCALL` entry for the hottest methods
  (`is_match`, `search`, `findall`).
  [acted — it does not: a frozen pyclass method with rurex's exact
  signature scales 47 → 40 ns at four workers, so the signature stays
  and the API is untouched.]
- [acted] `finditer` has the worst inflation (4.2×) because it creates a
  `Match` per match, which holds two `Py` handles; if the trampoline is
  not the cause, the `Match` handle pair is the next suspect.
  [acted — it is the next suspect, and it is now the "Python result
  objects on free-threaded Python" plan record (re-measured: 2.8× at
  four workers after the per-thread engine change).]
- [open] The `perf` evidence is per-symbol self time with frame-pointer
  call graphs; on the hybrid CPU (`cpu_atom`/`cpu_core` PMUs) plain
  `perf record` records almost nothing, `-e task-clock` is what works.

### Tooling/process

- `perf` is now installed on `regexus`; `perf_event_paranoid` is 1, so
  counting and sampling work for own processes. Sampling needs
  `-e task-clock`: the default event picks one of the two hybrid PMUs
  and misses most samples.
- Two probe mistakes cost time and are worth avoiding: `taskset -c 0`
  around a multi-worker probe defeats the per-worker pinning the
  benchmark does internally (`min_eff` collapses to ~0.35 and the point
  looks un-scalable), and a probe whose callable is a module global
  measures `LOAD_GLOBAL`/shared-refcount contention in the loop rather
  than the engine — hoist the callable and the slice into locals and
  keep the same loop shape for every engine being compared.
- Alternating wheels inside one lock session (as in the previous
  report) remains the only reliable A/B for per-call differences here:
  session-to-session drift on this host is larger than the effects being
  measured, and the project suite's own points swing 16–32% run to run.
- `git archive <commit> | tar -x` plus an rsync to the host is a cheap
  way to build a "before" wheel from an older commit without touching
  the working tree; it produced both the free-threaded and the GIL
  baseline wheels used here.
