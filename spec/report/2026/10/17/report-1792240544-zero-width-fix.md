## Report on task: zero-width iteration parity fix (CPython >=3.7)

Ad-hoc owner request: fix the corpus-suite's headline finding — after
an empty match, iteration advanced one unit and silently dropped the
non-empty match at that position (`re.findall(r'|b','abc')` returned
`['','','','']` instead of `['','','b','','']`), across `finditer`,
`findall`, `split`, `sub`/`subn` and the scanners.

### Done

- **Vendored PikeVM** (`src/vendor/pikevm.rs` + `sparse_set.rs`, from
  regex-automata 0.4.18, provenance header, every change marked
  `[rurex]`): added `Config::skip_empty` — an accept producing an
  empty match at the search start is dropped and lower-priority
  threads keep running, yielding the best *non-empty* match in
  leftmost-first preference order (exactly CPython's must-advance
  backtracking order). Upstream private-API adaptations documented in
  the header; `unexpected_cfgs`/`clippy::all` allowances for the
  frozen code.
- **`src/retry.rs`**: `RetryEngine` — anchored-at-`p`, skip-empty
  search with capture spans, built lazily per Pattern on the first
  empty match ever seen (patterns that never match empty never build
  or call it), mirroring the main engine's flag configuration and the
  regex crate's own str/bytes utf8 recipe. Rust unit tests pin the
  retry semantics (shadowed non-empty, lazy expansion, preference
  order, groups + case-insensitive).
- **Centralized CPython stepper** (`cpython_step` + `advance_after`
  in `src/regex.rs`): replaced the ten copy-pasted
  `scan = if sb == se { … }` advance sites (`FindIter::step`,
  `findall` scan loops, `split`, `substitute`, `SreScanner`, all in
  both str and bytes modes) with the must-advance state machine:
  empty match at `p` → anchored non-empty retry at `p` → on miss,
  advance one unit and continue normally.
- **Version-sync guard**: `vendor::tests` asserts Cargo.lock's
  `regex-automata` version equals the vendored file's recorded
  version — a regex bump turns red until re-vendored.
- **api.md**: zero-width section rewritten (old text documented the
  pre-3.7 rule); documents the one remaining known divergence.
- **Tests**: `tests/test_zero_width.py` (88 cases) — rurex == `re`
  across finditer/findall/split/sub (+ pos/endpos windows, bytes
  mode, scanner) on the fixed families; pins the documented
  divergence family.
- **Ledger re-triage**: `follow-up` rows 128 → 1 (only
  `bytes/invalid-utf8-anchor-200`, bug 2, remains). New by-design
  buckets: loop-capture (`test_bug_2537` family, spans equal),
  empty-body loop corner (`(?:|a)*`), `$`-end-only (pre-existing,
  cf. vendored `test_dollar_matches_twice`), `\B`-on-empty, POSIX
  `[:x:]` and `\b{start}` parse differences. Corpus suites 2,406
  green; pytest 279; vendored CPython suite unchanged-green; fmt,
  clippy, cargo test pass; release build restored.

### Known remaining divergence (documented, by-design)

Star-quantified empty-able bodies (`(|a)*`, `(a*)*`): CPython's
empty-iteration loop protection produces a third sequence that no NFA
engine reproduces (rust-lang/regex discussion #1164; the purpose-built
REAL engine documents the same three-way corner and also does not fix
it). rurex now reports the engine's leftmost-first results there —
same root phenomenon as the long-ledgered `test_bug_2537` capture
difference. Pinned in `tests/test_zero_width.py` and api.md.

### Spec/ADR amendments

- [acted] api.md zero-width section; rustregex EXPECTED.md ledger;
  report-1792235037 markers updated in this commit.

### Future-task notes

- [open] Bug 2 (unchanged): bytes-mode empty-match off-by-one on
  invalid UTF-8 (`bytes/invalid-utf8-anchor-200`) — last follow-up row.
- [open] Performance: the retry allocates a fresh PikeVM cache per
  call; if an empty-match-heavy workload ever shows up in benchmarks,
  switch to a `util::pool::Pool` per RetryEngine.
- [open] Upstream: rust-lang/regex discussion #1164 confirms
  must-advance is deliberately out of scope upstream; if that ever
  changes, the vendored PikeVM can be replaced by the public API.

### Tooling/process

- `#[cfg(feature = "syntax")]` in vendored upstream code resolves
  against *rurex's* features, silently compiling the builder methods
  out — strip foreign feature gates when vendoring, don't just add
  the dependency feature.
- The exact CPython rule, derived and verified empirically:
  must-advance is armed by every empty yield; a match is rejected iff
  empty *and* starting exactly at the previous end; the equivalent
  one-search-per-step formulation is "anchored skip-empty retry at p,
  else advance one unit" (the anchored retry must enumerate
  preference order past the empty preferred match — a plain anchored
  search recovers nothing).
- Edition 2024 rejects upstream's `ref mut` destructuring patterns
  and crate-internal `trace!` macros — both needed mechanical fixes
  when vendoring.
