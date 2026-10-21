# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Per-call cost on free-threaded Python

Status: new
Priority: 2

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
