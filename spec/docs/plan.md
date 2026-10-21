# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Python result objects on free-threaded Python

Status: new
Priority: 2

With per-thread engines in place (ADR-0019) the free-threaded
per-call floor is the Python result objects, not the engine: on 200k
Telegram posts, per-post wall cost of one worker at 1 versus 4 workers
is 1.04x for `is_match` (99 vs 96 ns), 1.8x for `findall` (302 vs 166)
and 2.8x for a `finditer` count (661 vs 243). A PyO3 method call that
creates nothing scales perfectly (`frozen` pyclass, rurex's exact
signature: 47 -> 40 ns), so the remaining loss is proportional to the
number of Python objects a call creates.

Targets, in the order the measurements point:

- `finditer`/`Match`: two `Py` handles per match (`Match.re`,
  `Match.string`) plus the pyclass itself. 2.8x at four workers.
- grouped `findall`: one `Vec<Bound>` plus a list per call, and one
  `bytes`/`str` per value.
- `sub`/`split` with a callable: same per-match objects through a
  Python callback.

Measurement recipe (this is what produced the numbers above, and the
previous session's contradictory results came from getting it wrong):
one case per *process*, worker threads created once, N barrier-started
rounds inside them, `measurement.pick_physical_cpus` plus
`pin_current` per worker, callables hoisted into locals, and the same
loop shape for every engine compared. Recreating threads per round
inflates every four-worker number two- to threefold.

References: ADR-0019, ADR-0018 (superseded),
`spec/report/2026/10/21/report-1792573419-free-threaded-per-call.md`

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
