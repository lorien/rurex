# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## `findall` does not scale on free-threaded Python (needs a decision)

Status: new
Priority: 2

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
