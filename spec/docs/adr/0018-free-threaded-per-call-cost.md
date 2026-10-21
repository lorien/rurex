# Free-threaded per-call cost: the engine pool is not the bottleneck

Date: 2026-10-21
Status: accepted

## Context

The plan record "Per-thread engines to remove free-threaded
cache-pool contention" assumed that rurex's free-threaded Telegram rows
stop scaling because all threads share one `regex::Regex`, whose
internal `CachePool` then serves every thread but its owner through a
mutex. regex-automata documents exactly that shape and prescribes
per-thread clones (each clone keeps the shared immutable program and
gets its own pool) or an explicit per-thread `Cache`.

The remedy was implemented: a per-thread table keyed by the pattern id
introduced in ADR-0017, holding a clone of the engine next to the
`CaptureLocations` scratch, with a second reserved id per pattern for
the anchored `fullmatch` engine, and every matching site driving the
thread's clone. It passed the full suite and behaved correctly under
eight threads. It was then measured and reverted. This record keeps the
measurements so the investigation is not repeated.

## Measurements

Benchmark host, core lease held, alternating wheels in one session,
`perf` available (installed for this work).

**Real Telegram corpus, `findall`, 4 workers** (3 alternating runs):

| | before | with per-thread engines |
|---|---:|---:|
| wall | 0.0289 s | 0.0296 s |
| cpu_core cycles | 22.54e9 | 22.34e9 |
| instructions | 22.43e9 | **21.25e9 (−5.3%)** |

The clone removes instructions (the pool's mutex path) but neither
cycles nor wall time.

**Per-call inflation on the free-threaded interpreter**, same 200k
posts, per-post wall time of one worker at 1 vs 4 workers:

| operation | 1 worker | 4 workers | inflation |
|---|---:|---:|---:|
| `len(t)` (builtin) | 34 ns | 39–40 ns | 1.15× |
| `t.count(b"@")` (C method, same buffers) | 124 ns | 146–148 ns | 1.2× |
| `re.search`, same pattern and data | 208 ns | 389–392 ns | 1.9× |
| `rurex.is_match` | 104–111 ns | 272–400 ns | 2.6–3.6× |
| `rurex.findall` | 178 ns | 623–639 ns | 3.5× |
| `rurex` `finditer` count (one `Match` per match) | 253 ns | 1061–1134 ns | 4.2× |

The inflation is not a property of the data or of Python object
creation: a C method over the same buffers grows 1.2×. It grows with
the number of Python-visible operations rurex performs per call.

**Where the 4-worker time goes** (`perf`, self time): `_Py_IncRef`
21.5%, `_Py_DecRefShared` 17.8%, `Py_DECREF` 11.9% — 51% in CPython's
shared reference-count paths, against 18% at one worker. rurex's own
frames are a small minority of the inflated total: `memchr` 6.2% and
the engine pool 5.8% at four workers (12.3% and 13.5% at one).

**A per-call haystack reference was tried too.** `HayKeep` used to
clone the `Bound` of the haystack (one INCREF/DECREF pair per call,
atomic on free-threaded CPython because the object usually belongs to
another thread). Borrowing it instead measured 111 → 104–106 ns per
`is_match` on the free-threaded build (W=1), but 100 → 109 ns on the
GIL build for a pattern without capture groups, and nothing at four
workers. Contradictory and small: reverted rather than shipped.

## Decision

1. **Do not isolate engines per thread.** The pool costs about 13% of
   rurex's own CPU at one worker and 5.8% at four; removing it changes
   instructions but not time, while costing ~1.5% per call single
   threaded (one more thread-local fetch and a clone-owned pool). It
   does not address the observed inflation.
2. **The free-threaded Telegram rows are bound by per-Python-call cost,
   not by the engine.** A call that creates no Python objects at all
   (`is_match`) already inflates 2.6–3.6× at four workers, and rurex's
   Rust frames account for ~12% of that. The remaining lever is the
   number of Python-visible operations per call and the cost of the
   PyO3 trampoline, not the matching engine.
3. **Keep the plan task, with that scope.** The record is rewritten to
   target the measured cause (see `plan.md`); its first step is a
   measurement of the trampoline itself rather than another engine-side
   change.

## Alternatives rejected

- **Per-thread engine clones.** Measured above: no wall-time effect,
  bounded but real single-thread cost, and a copy of every thread-local
  DFA/hybrid cache per thread (the engine's own size limits bound it at
  a few MB per thread and pattern).
- **An explicit per-thread `Cache` via `search_with`.** Same pool
  effect as the clone (removes the mutex), so it inherits the same
  result; it also means moving the matching layer onto
  `regex-automata`'s meta API, a rewrite for no measured gain.
- **Making the haystack borrow unconditional (no `Bound` clone).** The
  single-thread signal across the two interpreters disagrees and the
  four-worker signal is absent; not enough to justify carrying the
  `BytesKeep` enum for.
- **Documenting the rows as "not scaling" and stopping.** The
  decomposition above shows the loss is per-call interpreter/FFI work
  that grows with the number of Python operations, which is a binding
  concern; it deserves a scoped task, not a footnote.
