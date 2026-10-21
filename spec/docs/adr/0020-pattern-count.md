# `Pattern.count`: a counting entry point beyond `re`

Date: 2026-10-21
Status: accepted

## Context

Counting matches is a common shape, and the `re`-compatible spellings
both materialise objects the caller throws away: `len(rx.findall(s))`
builds one value per match, `sum(1 for _ in rx.finditer(s))` builds one
`Match` per match. On free-threaded CPython that cost is also the only
workload where rurex stops scaling: measured over 200k Telegram posts,
per-post wall cost of one worker goes 167 ns at one worker to 302 ns at
four for `findall` (1.8× worse per post), while stdlib `re`'s
`findall` keeps scaling (208 → 195 ns), and `is_match`, which creates no
objects, scales at 1.04×. `perf` puts 48% of `findall`'s four-worker
self time in CPython's shared reference-count paths against 16% at one
worker (report-1792581610-findall-scaling).

The engine and the binding had nothing left to give for `findall`
itself: per-thread engines (ADR-0019) fixed the scan, a thread-local
flat span buffer measured neutral, and borrowing the haystack instead of
cloning it measured −5% for `is_match` but +5% for `finditer` at four
workers. Creating no result object at all is the remaining lever, and it
needs a name in the API.

## Decision

1. **`Pattern.count(string, pos=0, endpos=None) -> int`**, and only
   that: one method on the compiled pattern, no module-level
   `rurex.count`. The module namespace stays exactly `re`'s (a module
   function would change `rurex.__all__`/`dir(rurex)`, which
   `tests/test_module.py` pins).
2. **The value is defined by the iteration**, not by a new matching
   mode: it equals `len(rx.finditer(string, pos, endpos))` — the same
   window, the same left-to-right advancement, and CPython's zero-width
   rule (the empty-match retry) included. `findall` returns one value
   per match too, so it equals `len(rx.findall(...))`.
3. **No Python object per match.** The whole scan runs in one Rust pass
   using the engine's `find_at` (group 0 only), one span on the stack,
   with the GIL released for a large window like `findall`'s. Groups are
   irrelevant to a count, so the capture machinery is not touched.
4. **Documented as an extension.** `api.md` lists it under the
   `Pattern` methods and in the "differences from `re`" section: `re`
   has no such name, and code that only uses `re`'s surface cannot
   notice it.

## Consequences

Measured on 200k Telegram posts, per-post wall cost of one worker
(three alternating rounds, one case per process, threads created once):

| call | 1 worker | 4 workers | inflation |
|---|---:|---:|---:|
| `is_match` (no window arguments) | 95 ns | 100 ns | 1.05× |
| `count(string)` | 123 ns | 254–263 ns | 2.05× |
| `count(string)` with a pattern that never matches | 84 ns | 227 ns | 2.7× |
| `findall` | 186 ns | 568–598 ns | 3.1× |
| `re` `finditer` count (for reference) | 456 ns | 1298 ns | 2.85× |

So `count` is 1.5× faster than `findall` at one worker and 2.3× at
four, and 3.7–5.1× faster than the `re` spelling of the same task. It
does not reach `is_match`'s scaling (1.05×), and the first explanation
tried — that the `pos`/`endpos` parameters in the PyO3 signature are the
cost — is **wrong**: measured in one binary, `count` with the window
parameters (130.7 ns at one worker, 272.0 at four) and a temporary
`count` without them (131.8 / 265.9) are the same, while `is_match`
(93.9 / 98.2) scales. The difference is therefore inside the call: the
iterate loop performs one search per match plus one final failing
search, where `is_match` performs a single search, and that second
search per post is what inflates. Isolating it is its own plan record,
with the same-binary comparison as the method: build both variants into
one binary, never compare across rebuilds (the same trivial probe method
measured 47 ns and 86 ns in two builds of the probe extension).

- The published benchmark rows do not change: rebar's models are
  engine-agnostic (`count` is `sum(1 for _ in finditer)`, others call
  `m.group(0)`), and the Telegram harness compares like with like, so a
  rurex-only method cannot be used in either without making the
  comparison unequal. The value is for users, not for the scoreboard,
  and this record says so rather than implying a row got fixed.
- `re` compatibility is additive: nothing that exists changes shape, no
  method is renamed, and `assert not hasattr(re.Pattern, "count")` holds
  today. The residual risk is a future CPython `Pattern.count` with
  different semantics; if that lands, rurex follows its definition.
- One test pins the surface decision: the name exists on `rurex.Pattern`,
  not on `re.Pattern`, and not on the module.

## Alternatives rejected

- **Module-level `rurex.count(pattern, ...)` as well.** Convenience only,
  and it changes the module namespace that `test_module.py` pins as
  exactly `re`'s; it can be added later as its own decision.
- **A keyword on `findall` (`count=True`).** Changes the return type of
  an `re`-compatible method and makes the type depend on an argument;
  strictly worse than a separate name.
- **No API change, leave `findall` as the only non-scaling row.** Then
  counting workloads keep paying for objects they discard, and the
  measured scaling loss stays documented but unfixable.
- **Naming it `matches()`.** Reads like `RegexSet.matches`, which
  returns ids rather than a number, and collides conceptually with
  iteration. `count` matches `str.count`'s semantics (non-overlapping,
  left to right) and is what the caller means.
