# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Full lone-surrogate parity for str haystacks

Status: new
Priority: 0

rurex transcodes non-ASCII `str` haystacks to UTF-8. CPython's encoder
rejects lone surrogates (U+D800..U+DFFF), so rurex raises
`UnicodeEncodeError` where stdlib `re` matches the in-memory string
normally. The cold-str prefilter returns the normal no-match answer
when the required first byte is absent, but haystacks that reach the
transcoder still raise. Choose and implement full parity: keep
surrogates through matching without corrupting offsets, or add a
UCS-aware path; add differential tests against `re`.

References: ADR-0012, ADR-0013

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
