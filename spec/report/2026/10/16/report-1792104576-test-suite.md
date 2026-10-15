## Report on task: Comprehensive Python test suite

### Task (archived from plan.md)

## Comprehensive Python test suite

Status: done

Fill `tests/` with pytest coverage of every public name in
`spec/docs/api.md`: compilation, all flags, all matching methods
including `prefixmatch` and `pos`/`endpos` windows, `RegexSet`
membership, Match accessors, error cases, `str`/`bytes` modes, and
differential cases comparing against the stdlib `re` where
semantics are shared (including `\A` anchoring and zero-width-match
behavior).

References: ADR-0002, ADR-0004

### Done

- `tests/test_differential.py`: corpus sweep (18 str + 5 bytes
  pattern/text pairs incl. anchors, alternation, named groups,
  quantifiers, unicode) through every method pair against `re`
  (search family spans+groups, findall/finditer, split at two
  maxsplits, sub with template and callable, is_match); a flags
  matrix (I/M/S/X/A, combinations) on flag-sensitive patterns; a
  `pos`/`endpos` matrix sweeping every pos in range and four
  endpos values over five patterns and four texts for
  search/prefixmatch/finditer.
- Deviation tests per api.md bullet: `\Z` error vs `\z`, `$` and
  trailing newline (`\n?$` emulation), octal escapes rejected,
  `LOCALE` rejected on both types, no `match` alias, `(?U)`
  swap-greed semantics; Match unpicklable parity; finditer is its
  own iterator.
- `escape` differential over punctuation/whitespace/unicode plus
  bytes, and a type error case.
- `tests/test_threads.py`: concurrency smoke for the GIL-release
  paths (shared Regex across threads, module functions through the
  cache, and a 2 MB scan parallel with other work).
- **Two real bugs found by the matrices and fixed in `src/regex.rs`:**
  1. `endpos < pos` (after length clamping) returned matches
     instead of none — `window()` now implements CPython's exact
     rule (clamp both to length, then endpos < pos is no match);
     finditer returns an exhausted iterator in that case.
  2. `prefixmatch` anchored at the raw `pos`, not the
     length-clamped one (`match('x?', 'ab cd', 6)` must match at
     (5,5) like CPython).
- Feature fold-in (as proposed): `rurex.escape` now accepts
  `str | bytes` mirroring `re.escape` (own byte-level escaper with
  CPython's special-character set — `regex::bytes::escape` does not
  exist; the set also replaced `regex::escape` for parity).
- Suites: 94 pytest + 20 Rust green; fmt/clippy clean.

### Spec/ADR amendments

- api.md: escape signature `str | bytes`; the `pos`/`endpos`
  clamping and endpos-below-pos rule documented. [acted]
- plan.md task archived; the scaffold report's `escape`-str-only
  marker marked [acted]. [acted]

### Future-task notes

- [open] The corpus is deterministic and modest; a randomized
  (seeded) differential fuzzer over generated patterns would be the
  natural next hardening step if drift ever appears.
- [open] The `fullmatch` fallback path (anchored variant fails to
  build, e.g. nest-limit patterns) remains untested — constructing
  a triggering pattern is impractical; revisit if the fallback is
  ever observably hit.

### Tooling/process

- CPython's window rule was derived empirically under the matrix:
  clamp pos and endpos to the text length first, then endpos < pos
  means no match — including the subtle case where pos beyond the
  length clamps down to endpos and empty matches still succeed.
- Stdlib 3.13 quirk in differentials: `Pattern.split`/`subn` warn
  on positional maxsplit/count; tests pass them as keywords.
