# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Window arguments cost 2.3x on free-threaded Python

Status: new
Priority: 2

Every windowed method (`search`, `match`/`prefixmatch`, `fullmatch`,
`findall`, `finditer`, `count`) is declared as
`#[pyo3(signature = (string, pos = 0, endpos = None))]`, and that shape
is what stops them from scaling on free-threaded CPython. Measured with
the probe extension (ADR-0019, ADR-0020), frozen pyclass, per-process,
threads created once, per-post wall of one worker:

| probe method | 1 worker | 4 workers |
|---|---:|---:|
| `m(string)` | 47 ns | 41 ns (1.0x) |
| `m(string, pos = 0, endpos = None)` | 55 ns | 124 ns (2.3x) |
| `m(string, pos)` with `pos` required | 69 ns | 246 ns (3.6x) |

The same pattern shows in rurex itself: `is_match(string)` scales 1.05x
at four workers while `count(string, pos=0, endpos=None)` inflates 2.7x
even with a pattern that never matches, and 51% of that four-worker
profile is CPython's shared reference-count paths. A supplied integer
argument is worst because callers pass small ints, which are shared
singletons: four threads then hammer one object's refcount line.

The task: make the window parameters free on the hot path, without
changing the public signature (`re` accepts `pos`/`endpos` both
positionally and as keywords, so they stay). Candidates, cheapest to
try first:

1. Hand-written argument handling: take `*args, **kwargs` and parse the
   two optional ints in Rust, bypassing PyO3's generated
   `extract_arguments_fastcall` for the common `rx.method(text)` and
   `rx.method(text, pos)` calls.
2. A split surface: keep the keyword-capable method, and give the
   common no-window call a plain one-argument entry point that the
   keyword form delegates to.
3. If neither moves it, check whether PyO3 0.31+ changed the defaults
   code path, and whether `Option<usize>` (Rust default `None`) is
   cheaper than `usize` with a default.

Constraint: semantics and the public signature must not change; the
measurement recipe from ADR-0019 applies (one case per process, threads
once, `perf stat -e task-clock,instructions` for anything under 10%).

References: ADR-0020, ADR-0019,
`spec/report/2026/10/21/report-1792581610-findall-scaling.md`

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
