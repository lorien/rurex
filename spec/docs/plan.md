# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

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
