# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## A match loop inflates 2x where a single search does not

Status: new
Priority: 2

`Pattern.count` is 1.5x faster than `findall` at one free-threaded
worker and 2.3x at four, but it still inflates 2.0x per post at four
workers where `is_match` inflates 1.05x. Measured in **one binary**
(200k Telegram posts, per-process, threads created once):

| call | 1 worker | 4 workers | inflation |
|---|---:|---:|---:|
| `is_match(string)` | 93.9 ns | 98.2 ns | 1.05x |
| `count(string, pos=0, endpos=None)` | 130.7 ns | 272.0 ns | 2.08x |
| `count` with the window parameters removed (temporary probe) | 131.8 ns | 265.9 ns | 2.02x |

So the window arguments are **not** the cost (the earlier 2.3x claim came
from comparing across rebuilds of a probe extension, where the same
trivial method measured 47 ns and 86 ns in two builds — never compare
across builds for effects under ~50%). What differs is the call's shape:
`is_match` runs one search per post, while the iterate loop runs one
search per match plus one final failing search, and that extra search
costs ~4x more at four workers than at one. `count` on a corpus where
the pattern never matches is still 2x worse at four workers, so the
effect is present even with exactly one search — the difference is
therefore in how the *loop* drives the engine (`find_at` from a moving
start) rather than in the per-match work.

Next: profile `count` with a never-matching pattern (`zzzzzq`) at 1 and 4
workers and compare against `is_match` on the same data — both do one
search, so any difference is in the call path (`with_*_engine` borrow,
`cpython_step`, `find_at` versus `is_match`). Then decide whether the
same shape can be used for `is_match`/`search`, or whether the iterate
loop's `find_at` can avoid whatever the pool/DFA does differently.

Method: one case per process, worker threads created once, N
barrier-started rounds, `measurement.pick_physical_cpus` plus
`pin_current` per worker, callables hoisted into locals, variants built
into the *same* binary, and `perf stat -e task-clock,instructions` for
anything under 10%.

References: ADR-0020, ADR-0019,
`spec/report/2026/10/21/report-1792589624-pattern-count.md`

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
