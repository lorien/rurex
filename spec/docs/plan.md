# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Cheaper per-match objects for finditer-heavy workloads

Status: new
Priority: 2

rebar's `08-words/all-english` (byte mode, 56,601 matches in 2,500
subtitle lines) measures 3.56 ms for rurex against 2.25 ms for stdlib
`re` and 0.71 ms for the `rust/regex` engine alone. The gap is not the
scan: with that many matches, the cost is one Python object per match,
about 63 ns per match for rurex against 40 ns for `re` and 13 ns for the
engine's own loop. Every `finditer`/`count`-shaped workload pays it, and
it is the largest single reason rurex trails `re` on a benchmark family
that is otherwise scan-bound.

`Match` currently allocates a `Vec` for its spans (plus the pyclass
itself) and clones an `Arc` of capture names. The candidate fixes are
those already noted in the result-path report: an inline array for the
usual 0–3 groups with a heap fallback, and a thread-local span buffer
returned on drop, measured against rebar's words benchmarks and the
existing `finditer` microbenchmarks.

Not part of this task: the Russian variants of that group. There the
engine itself is slower than `re` (6.07 ms against 3.03 ms) because its
DFA cannot handle a Unicode-aware `\b` on non-ASCII bytes, and rurex's
transcode of the 130 KB haystack is under 1% of the call. That is an
upstream engine limitation, recorded on the rebar page, not something
the binding can fix.

References: ADR-0008,
`spec/report/2026/10/18/report-1792364009-result-path-offsets.md`,
`spec/report/2026/10/19/report-1792446048-rebar-comparison.md`

## Reuse `CaptureLocations` instead of allocating `Captures` per call

Status: new
Priority: 2

Every `search`/`match`/`fullmatch`/`finditer` step calls
`regex::Regex::captures_at`, which allocates a `Captures` object and its
location vector per call; grouped `findall` does the same per match. That
is the largest remaining item in the text `search` gap to `re`: the
result-path report measured text `search` at ~3,500 instructions per call
on a short haystack, within ~14% of `re`, with this allocation a
substantial share of what is left.

`Regex::captures_read_at` writes into a caller-provided
`CaptureLocations`, so the fix is to keep one per engine per thread
(thread-local, keyed by a stable engine id so a freed engine's slot is
never reused) and copy the spans out of it. The engine's `Regex` is
already shared across threads, so the storage must be thread-local rather
than a field.

References: ADR-0006,
`spec/report/2026/10/18/report-1792364009-result-path-offsets.md`,
`spec/report/2026/10/18/report-1792325206-findall-prefilter-speedup.md`

## Per-thread engines to remove free-threaded cache-pool contention

Status: new
Priority: 2

On free-threaded CPython the Telegram bytes row stops scaling (0.16 s at
one worker, 0.16 s at four, against `re`'s 0.10 s at four), and the
contamination-guard report names the shared `regex` cache pool as the
cause. regex-automata's own documentation describes exactly this shape —
"the same `Regex` is shared across multiple threads … searches are run on
very short haystacks" — and prescribes either cloning the `Regex` per
thread (each clone keeps the shared immutable program but gets its own
pool) or using `search_with` with an explicit per-thread `Cache`.

Either route needs a thread-local store keyed by a stable engine id, the
same mechanism as the `CaptureLocations` task, so the two are worth doing
together. Verify on the benchmark host with the guard active; single
thread behaviour must not change.

References: `spec/report/2026/10/18/report-1792361198-contamination-guard.md`,
`spec/report/2026/10/18/report-1792364009-result-path-offsets.md`,
rust-lang/regex#934

## Wire the cold-str prefilter into `split`/`sub`/`subn`

Status: new
Priority: 2

ADR-0013's required-first-byte prefilter covers `findall`, `finditer`,
`search`, `match`/`prefixmatch`, `fullmatch` and `is_match`, but not
`split`/`sub`/`subn`: those still transcode every cold non-ASCII `str`
before discovering there is nothing to do. A no-match answer there has to
reproduce `re`'s output exactly — a plain `str` (or `bytes`) copy of the
input, including subclass behaviour — so the work is to make the
"nothing matched" answer a copy of the original object rather than the
result of a scan, then gate the transcode on the same prefilter.

The `CaptureLocations` part of grouped `findall` is tracked in its own
record; this one is only the prefilter wiring.

References: ADR-0013,
`spec/report/2026/10/18/report-1792325206-findall-prefilter-speedup.md`
