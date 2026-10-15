## Report on task: Implement RegexSet

### Task (archived from plan.md)

## Implement RegexSet

Status: done

Implement `rurex.compile_set(patterns, flags=0)` returning a
`RegexSet` wrapping `regex::RegexSet`: `is_match(text) -> bool` and
`matches(text) -> list[bool]`, membership only, flags applied to all
patterns. Coverage for empty pattern list, result ordering, and
`str`/`bytes` modes.

References: ADR-0004

### Done

- `src/regexset.rs`: frozen `RegexSet` pyclass over
  `regex::RegexSet` / `regex::bytes::RegexSet`; `compile_set`
  collects from any Python iterable (one-shot generators included —
  items are collected in the first pass), enforces a single mode
  from the first item (`TypeError: patterns must be all str or all
  bytes`), allows the empty set (engine-supported; never matches),
  applies `flags::translate` with the same `ValueError`s as
  `compile`, pre-parses each pattern for full `rurex.error`
  attribute parity (`pattern`/`pos` point into the individual
  failing pattern), and builds via `RegexSetBuilder` under the
  translated config.
- Methods: `is_match(text) -> bool` and `matches(text) -> list[bool]`
  (one boolean per pattern, in order), text validation reused from
  `extract_hay` (same `TypeError` messages as `Regex`), GIL released
  around both scans.
- Refactor in `src/regex.rs`: the regex-syntax pre-parse block moved
  into a shared `pub(crate) validate_pattern`; `extract_hay`/`Hay`
  made `pub(crate)`.
- `src/lib.rs`: `compile_set` function and `RegexSet` class
  registered; surface guard updated.
- Tests: `tests/test_regexset.py` (10 tests) — membership and
  ordering, overlapping hits, empty set, flags applied to all,
  generator input, bytes mode, flag errors mirroring `compile`,
  per-pattern error attributes, all TypeError paths, non-UTF-8
  bytes patterns with `\xNN` escape alternative. Suites: 63 pytest +
  20 Rust unit tests green; fmt/clippy clean.

### Spec/ADR amendments

- api.md RegexSet section expanded: mode rule and mixed-type
  `TypeError`, empty-list allowance, per-pattern error attributes,
  text-mode `TypeError`s. [acted]
- plan.md task archived here. [acted]

### Future-task notes

- [open] If `RegexSet` ever grows offsets (e.g. `which_matches` via
  `SetMatches::iter`), revisit whether `pos`/`endpos` should come
  with it; engine `matches_at` already exists for a start offset.

### Tooling/process

- Collecting items in the first iteration pass (instead of
  re-iterating) is what makes generator inputs work; worth
  remembering for any future iterable-accepting API.
- One test of ours was initially wrong about raw-byte matching
  (`a\xffb` cannot match `\xff`); the differential habit of testing
  the escape form separately caught it.
