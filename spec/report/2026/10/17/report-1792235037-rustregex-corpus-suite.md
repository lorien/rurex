## Report on task: rust-regex corpus conformance suite (layers A+B)

Ad-hoc owner request: vendor rust-regex's testdata and run it against
rurex, plus run stdlib `re` on the same inputs to confirm rurex and
`re` behave identically on identical data.

### Done

- Vendored rust-lang/regex `testdata/` at tag `1.13.1` (matches the
  `Cargo.lock` regex version; commit `2b527599eb9e`) into
  `tests/rustregex/data/` — 1,203 cases, licenses, `PROVENANCE.md`
  with re-download command and the bump rule.
- Harness (`tests/rustregex/`): `corpus.py` (TOML loader, span
  normalization, byte↔char conversion, `\xNN` unescaping),
  `engine.py` (layer A driver), `parity.py` (layer B driver),
  `expected.py` + `EXPECTED.md` ledger (788 bucketed rows),
  `gen_expected.py` (ledger regeneration with classification rules),
  `vendored_corpus_engine.py` / `vendored_corpus_parity.py` (pytest
  entry points, 2,406 parametrized tests, ~3 s). Not collected by a
  plain `pytest` run — explicit invocation like the CPython suite.
- Layer A — binding conformance: rurex vs corpus expectations
  (options mapping: `case-insensitive`→IGNORECASE, `anchored`→
  `.match`, `bounds`→`pos`/`endpos`, `unescape`→bytes mode,
  multi-pattern→`compile_set` membership).
- Layer B — parity: rurex vs stdlib `re` on identical inputs; both
  must agree on compile verdict and full match iteration.
- Ledger discipline enforced both directions (missing row fails;
  stale row fails). CI: new step on the python 3.13 cell only (layer
  B is `re`-version-sensitive). Docs: testing.md section, ADR-0010,
  index entry. Final checks pass; release build restored afterwards.

### Findings (first run)

- **[follow-up] Zero-width iteration parity defect.** `FindIter::step`
  advances one character after an empty match; CPython ≥3.7 instead
  retries at the same position and yields a following non-empty
  match. Reproducer: `re.finditer(r'|b','abc')` →
  `(0,0),(1,1),(1,2),(2,2),(3,3)`; rurex omits `(1,2)`. ~50 layer-B
  rows (`follow-up` bucket) plus layer-A rows where rurex matches
  neither engine nor CPython. Affects `finditer`/`findall` and likely
  `sub`. Note: `api.md` currently *documents* the old behavior —
  fixing means a semantic change plus api.md update (see
  [needs-decision] below).
- **[needs-decision] Zero-width semantics direction.** Adopt CPython
  ≥3.7 alternation semantics (recommended: parity is the product
  promise, and 30 corpus cases already follow CPython against the
  engine), or keep the documented advance-by-one and re-bucket the
  ledger rows as by-design. Owner call.
  [acted — adopted CPython ≥3.7 semantics: vendored PikeVM with an
  empty-accept skip drives a shared must-advance retry; fixed in
  report-1792240544-zero-width-fix.]
- **[follow-up] bytes-mode empty-match off-by-one on invalid UTF-8**
  (`bytes/invalid-utf8-anchor-200`, upstream fuzz regression case):
  rurex reports the empty match at byte 23, engine at 22.
- **[needs-decision] Corpus ahead of pinned crate:** mid-pattern
  global flags (`(?:(?i)foo)|Bar`) are expected to compile by the
  corpus but the pinned 1.13.1 parser — and `re`, and rurex — reject
  them. Watch when `Cargo.lock` bumps regex; re-triage then.
- Not defects: `RegexSet` reports membership while the corpus's
  leftmost-first set tests expect alternation-priority selection
  (unmapped by design, ADR-0004 membership-only surface); empty-loop
  capture and `\B`-on-empty-string divergences are by-design engine
  semantics already ledgered in the CPython suite (cf. test_bug_2537).

### Spec/ADR amendments

- [acted] spec/docs/adr/0010-rustregex-corpus-suite.md (new),
  index.md entry, testing.md section — this session.

### Future-task notes

- [open] Implement the zero-width iteration decision above; when
  done, re-run `gen_expected.py`, re-bucket the `follow-up` rows, and
  drop the corresponding EXPECTED entries in the same commit.
  [acted — implemented in report-1792240544-zero-width-fix; ledger
  re-triaged, follow-up rows reduced from 128 to 1 (the bytes-mode
  off-by-one below).]
- [open] Triage the `follow-up` "zero-width/empty-loop needs triage"
  subrows (29 cases) into step-bug vs capture-semantics buckets once
  the decision lands.
  [acted — triaged into by-design buckets: loop-capture (spans equal,
  test_bug_2537 family), empty-body loop corner, $-end-only, and
  \B-on-empty; see the regenerated EXPECTED.md.]
- [open] When `Cargo.lock` regex bumps: re-vendor `testdata/` at the
  matching tag, re-run `gen_expected.py`, diff the ledger (behavior
  drift shows up as new unledgered failures) — per PROVENANCE.md.
  [amended: since the zero-width fix, also re-vendor
  `src/vendor/pikevm.rs` at the matching regex-automata version and
  re-apply the marked [rurex] guard; a unit test enforces the sync.]
- [open] sdist grows by ~290 KB of vendored TOML corpus; if wheel
  phase 1's "sdist is fat" item is ever acted on, keep the corpus in
  the repo but consider excluding `tests/rustregex/data/` from the
  sdist.

### Tooling/process

- Layer-B parity drivers must not compare against corpus expectations
  — only rurex vs `re` — or every engine-syntax case would file as a
  failure; the corpus is ground truth for layer A alone.
- stdlib `re` patterns take integer `endpos` defaults (not `None`);
  passing `endpos=None` raises TypeError after compile succeeds —
  silent harness trap when reusing rurex call shapes.
- The corpus TOML uses `\xNN` in single-quoted strings; the escape
  decoder must skip the `x` (slice `[i+2:i+4]`) — an off-by-one there
  shifts every bytes-mode haystack and fails ~10 cases confusingly.
