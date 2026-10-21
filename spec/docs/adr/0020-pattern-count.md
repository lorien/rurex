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
four, and 3.7–5.1× faster than the `re` spelling — but it does *not*
scale like `is_match`. The profiling says why: with a pattern that never
matches, so that no per-match work happens at all, `count` still
inflates 2.7×, and 51% of its four-worker self time is CPython's shared
reference-count paths. The difference from `is_match` is the argument
list: `count` carries `pos = 0, endpos = None`, and a PyO3 method with
defaulted integer parameters inflates ~2.3× at four workers where a
one-argument method on the same frozen class scales 1.0× (a *supplied*
integer argument, which the caller passes as a shared small-int object,
is worse still: 3.6×). That is a property of every windowed method in
the binding (`search`, `match`, `fullmatch`, `findall`, `finditer`,
`count`), not of the counting path, and it is the next lever — with its
own record in `plan.md`, since fixing it would lift all of them.
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
