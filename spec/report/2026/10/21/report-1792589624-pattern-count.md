## Report on task: Pattern.count (feature) and the findall-scaling record

Owner decision: add `Pattern.count(string, pos=0, endpos=None) -> int`,
method only, as the one surface extension beyond `re`. The plan record
that motivated it ("`findall` does not scale on free-threaded Python")
is archived here as done, and its remaining lever became a new record.

### Task (archived from plan.md)

## `findall` does not scale on free-threaded Python (needs a decision)

Status: done

Measured on 200k Telegram posts, per-post wall cost of one worker at 1
versus 4 free-threaded workers (one case per process, threads created
once, per-worker pinning: the recipe below):

| engine | op | 1 worker | 4 workers |
|---|---|---:|---:|
| rurex | `findall` | 167 ns | 302 ns (1.8x worse) |
| `re` | `findall` | 208 ns | **195 ns (scales)** |
| rurex | `finditer` count | 250 ns | 679 ns (2.7x) |
| `re` | `finditer` count | 317 ns | 1075 ns (3.4x) |

So the per-`Match` path already scales better than `re`'s (rurex is
ahead at both worker counts), but `findall` is the one workload where
`re` keeps scaling and rurex degrades -- and that is the Telegram bytes
row. The engine is not the cause (ADR-0019: `is_match` scales 1.04x);
at four workers `perf` puts 48% of `findall`'s self time in CPython's
shared reference-count paths against 16% at one.

Levers measured and rejected in this session:

- Reusing the flat span buffer across calls (thread-local `Vec`) instead
  of allocating it per call: no measurable change at either worker count
  (290-308 ns before, 288-301 after).
- Borrowing the haystack instead of cloning its `Bound` (one foreign
  refcount pair per call, which `re` never pays because a C method takes
  its argument borrowed from the vectorcall array): `is_match` 94 -> 88
  ns at one worker and 99 -> 93 at four, `findall` 166 -> 160 and
  296 -> 293, but `finditer` 678 -> 721 at four workers (5% worse,
  consistently over five alternating rounds, while executing 1% fewer
  instructions). Mixed enough that it was reverted; if it is ever
  revisited, split the iterator path from the rest.

What is left needs an owner decision, because it changes the API
surface rather than the implementation:

1. A counting entry point that returns an integer instead of a list of
   values (`rurex.count(pattern, string)` or `Pattern.count`), the shape
   a scan-only workload actually wants. `re` has no equivalent; the
   `re`-compatible `len(findall(...))` is what benchmarks (and users)
   write today, and it is exactly the object churn that does not scale.
2. Or accept the row: rurex is faster than `re` single threaded on this
   workload (167 vs 208 ns/post) and slower at four workers, and the
   binding cannot avoid building the list `findall` promises.

Measurement recipe (the +-2-3x errors in earlier sessions came from
getting this wrong): one case per *process*, worker threads created
once, N barrier-started rounds inside them,
`measurement.pick_physical_cpus` plus `pin_current` per worker,
callables hoisted into locals, the same loop shape for every engine
compared, and `perf stat -e task-clock,instructions` for a work-volume
cross-check on any wall-clock difference under 10%.

References: ADR-0019, ADR-0018 (superseded),
`spec/report/2026/10/21/report-1792576652-per-thread-engines.md`

### Done

- **Added `Pattern.count(string, pos=0, endpos=None) -> int`** on the
  compiled pattern and nowhere else: no module-level `rurex.count`, so
  the module namespace stays exactly `re`'s (which `tests/test_module.py`
  pins). Its value is defined by the iteration — it equals
  `len(finditer(...))` and `len(findall(...))` for the same window,
  including CPython's zero-width rule — and the whole scan runs in one
  Rust pass with the engine's `find_at` (group 0 only), one span on the
  stack, GIL released for a large window. No `Match`, no capture group
  and no result value is created.
- **Fixed a pre-existing panic the new tests found** (separate commit
  `b109fd3`): a zero-width match at the last position of a window that
  ends inside multi-byte text advances to the next character of the
  whole string, which can lie past the window;
  `find_at`/`captures_read_at` panic on such a start, so
  `rurex.compile("").finditer("héllo wörld 造字", 0, 12)` raised
  `PanicException` instead of `re`'s 13 matches. Every engine lookup
  that can receive an iterate advance is now bounded, and the multibyte
  case is covered against `re` for both `finditer` and `findall`.
- Wrote 10 differential tests (`tests/test_count.py`) covering the
  window matrix against both `finditer` and stdlib `re`, keywords,
  bytes mode and bytes-like haystacks, the GIL-released path, error
  parity, and the surface decision itself.
- Docs: `api.md` (method + a "differences from `re`" entry), ADR-0020,
  `architecture.md`, `index.md`; the plan record is replaced by the
  measured follow-up.

### Results

Telegraph corpus (200k posts, per-post wall cost of one worker, three
alternating rounds, one case per process, threads once):

| call | 1 worker | 4 workers | inflation |
|---|---:|---:|---:|
| `is_match` (no window arguments) | 95 ns | 100 ns | 1.05× |
| `count(string)` | 123 ns | 254–263 ns | 2.05× |
| `count` with a never-matching pattern | 84 ns | 227 ns | 2.7× |
| `findall(string)` | 186 ns | 568–598 ns | 3.1× |
| `re` `finditer` count (reference) | 456 ns | 1298 ns | 2.85× |

`count` is 1.5× faster than `findall` at one worker, 2.3× at four, and
3.7–5.1× faster than the `re` spelling of the same task. It does **not**
reach `is_match`'s scaling, and the reason turned out to be the most
useful finding of the session: with a never-matching pattern — no
per-match work at all — it still inflates 2.7×, so the cost is per
*call*, and the only difference from `is_match` is the argument list.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0020-pattern-count.md`: the extension, its
  definition by the iteration, the measured consequences (including the
  fact that the published benchmark rows are unaffected) and the
  rejected alternatives (module-level function, `findall(count=True)`,
  `matches()`).
- [acted] `spec/docs/api.md` lists `Pattern.count` and records it in
  "differences from `re`" as an addition, not a behaviour divergence.
- [acted] `spec/docs/architecture.md` matching path documents the
  counting path and why it is the one entry point whose cost does not
  grow with the number of matches.
- [acted] `spec/docs/index.md` catalogs ADR-0020.
- [acted] `spec/docs/plan.md`: this record is archived; a new record
  ("Window arguments cost 2.3x on free-threaded Python") carries the
  signature-shape measurements.

### Correction (same session): the window arguments are not the cost

The "signature" explanation recorded above was refuted before the
session ended, by measuring both variants in **one** binary: `count`
with `pos`/`endpos` (130.7 ns at one worker, 272.0 at four) and a
temporary `count` without them (131.8 / 265.9) are identical, while
`is_match` (93.9 / 98.2) scales. The earlier 2.3×/3.6× numbers came from
comparing across rebuilds of the probe extension, where the same trivial
method measured 47 ns in one build and 86 ns in the next — build-to-build
layout noise is larger than the effect. The plan record is now
"A match loop inflates 2x where a single search does not", with the
same-binary comparison as its method.

### Future-task notes

- [open] The new record's subject: `count` runs one search per match
  plus a final failing one, `is_match` runs a single search, and the
  iterate loop inflates 2.0× at four workers while `is_match` inflates
  1.05× — including with a pattern that never matches, so it is the call
  path, not the per-match work. Next step is a profile of the
  never-matching case against `is_match` on the same data.
- [open] `count` returns the number of *matches*, and `findall` returns
  one value per match even for grouped patterns, so `count` equals
  `len(findall(...))` for every pattern. A future grouped-count
  (`sum` of something) is a different request and not implied.
- [open] The panic fixed here came from the *text* path's advance
  stepping through the whole string while the search is bounded by the
  window. If a future change makes the window a slice of a different
  buffer (a transcode scratch), the same invariant must hold: an engine
  start is always ≤ the window length.

### Tooling/process

- The panic was found by a *differential test written for a new
  feature* (count == len(finditer) over a window matrix), not by the
  feature itself. New entry points are a cheap way to exercise the
  shared iteration machinery; the matrix is worth reusing.
- Splitting a bug fix out of a feature branch: `git reset --soft`,
  restoring the file to the fix-only state, and keeping the feature
  version aside as a file copy worked, but the order matters — running
  `pytest tests/` before `git add -A` would have caught that the
  feature's tests were still in the tree (they were briefly committed
  with the fix and had to be amended out).
- The perf evidence only became interpretable after adding the
  never-matching-pattern control: it separates per-call from per-match
  cost in one command.
