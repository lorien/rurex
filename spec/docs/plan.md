# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Faster pattern compilation: stop parsing twice

Status: new
Priority: 0

rebar's `compile` model shows rurex at 1.4–2.5× the engine's compile
time and, on some patterns, slower than stdlib `re`:
`03-date/compile-unicode` 8.17 ms against `rust/regex` 5.70 ms and `re`
3.75 ms; `03-date/compile-ascii` 3.62 against 1.46 ms;
`10-bounded-repeat/compile-capitals` 75.7 µs against `re`'s 27.3 µs;
`04-ruff-noqa/compile-real` 92.2 against 62.4 µs.

`Regex::build` parses the pattern with `regex-syntax` to produce
`re`-compatible errors (`pos`, `lineno`, `colno`), then hands the text
to the `regex` crate, which parses and translates it again. The
candidate fix is to compile first and run the `regex-syntax` pass only
when the engine rejects the pattern, keeping the textual checks that
enforce `re` rejections (stacked quantifiers, group-name rules,
mid-pattern global flags) in both paths.

This matters for dynamically built patterns and short-lived processes;
for a long scan it is noise. Parity is the risk: nothing the engine
accepts may become accepted where `re` rejects, so the conformance and
divergence suites decide whether the change ships, and rebar's `compile`
model is the benchmark that proves it.

References: ADR-0002, ADR-0006,
`spec/report/2026/10/19/report-1792446048-rebar-comparison.md`

## Cheaper per-match objects for finditer-heavy workloads

Status: new
Priority: 0

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
