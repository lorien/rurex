# Vendored rust-regex corpus: binding conformance and parity suites

Date: 2026-10-17
Status: accepted

## Context

The vendored CPython suite (ADR-0009) probes rurex's Python-visible
API surface. It does not systematically exercise the glue between
that API and the wrapped engine: flag translation (`src/flags.rs`),
the hand-written `match`/`fullmatch` anchored pattern wrappers, the
`VERBOSE` pattern wrap, `pos`/`endpos` byte-window slicing, byte→char
span conversion, `FindIter::step` advancement, and the dual
str/bytes paths. Upstream rust-lang/regex CI tests the engine through
its own Rust harness — none of rurex's binding layer runs there.

The rust-lang/regex repository ships its conformance corpus as
machine-readable TOML (`testdata/`, 1,203 cases: expected match spans
in byte offsets, capture-group spans, compile verdicts, and options
such as `anchored`, `bounds`, `case-insensitive`, `unescape`). It is
the only external corpus whose expectations match the semantics
rurex actually exposes (ADR-0002: Rust engine semantics underneath);
RE2's corpus expects longest/alternation-selection semantics, PCRE2's
expects backtracking-only features, mrab-regex's expects Python+
extensions.

## Decision

1. Vendor `testdata/` from rust-lang/regex at the tag matching the
   `Cargo.lock` regex version (1.13.1 at vendoring, 2026-10-17),
   verbatim, with licenses and a `PROVENANCE.md` (tag, commit,
   re-download command). When `Cargo.lock` bumps the regex version,
   re-vendor at the matching tag and re-triage the ledger — same
   rule as the CPython suite's pinned tag.
2. Run the corpus in two layers under `tests/rustregex/`:
   - **Layer A — binding conformance**
     (`vendored_corpus_engine.py`): rurex's Python-visible results vs
     the corpus's expected spans; str-mode byte spans are converted
     to char offsets; `unescape` cases run in bytes mode where spans
     compare directly; multi-pattern cases run as `compile_set`
     membership checks.
   - **Layer B — parity** (`vendored_corpus_parity.py`): for every
     case both modules can express, compile the same pattern with the
     same flags in rurex and stdlib `re` and require identical
     behavior — compile verdict plus the full match iteration
     (spans of every capture group, `None` for non-participants).
     Corpus expectations are irrelevant to this layer; a rurex/`re`
     disagreement is a bug or a ledger row.
3. `EXPECTED.md` ledger, ADR-0009 discipline, enforced in both
   directions by the harness: every non-passing case has a bucketed
   row (unmapped / by-design: python-parity / by-design:
   engine-semantics / by-design: rurex superset / corpus-ahead-of-
   pinned-crate / follow-up), and a row whose case passes is stale
   and fails the suite. `gen_expected.py` regenerates the ledger
   from a triage run; its classification rules are reviewed by hand.
4. Not collected by a plain `pytest` run (explicit invocation, like
   the CPython suite). CI runs both layers on the python 3.13 cell
   only: layer B compares against stdlib `re` and is triaged on one
   interpreter version; other cells would churn the ledger without
   new information.
5. First-run findings are recorded as the ledger's `follow-up` rows
   and in the session report — most notably a zero-width iteration
   parity defect (`FindIter::step` advances by one after an empty
   match; CPython ≥3.7 instead retries a non-empty match at the same
   position), plus a bytes-mode empty-match off-by-one on invalid
   UTF-8.

## Consequences

- Engine upgrades become auditable from Python: a corpus re-run
  after a `Cargo.lock` regex bump diffs behavior as rurex users see
  it, before a wheel ships.
- The corpus exercises constructs `re` lacks (`\p{…}`, POSIX classes,
  `(?-u)`, `\b{start}`, `(?R)`), where rurex is deliberately a
  superset; those cases pin the parity boundary instead of failing
  silently.
- Triage cost is real but bounded: 788 ledger rows at first triage,
  dominated by unmapped corpus options (`unicode=false` str mode,
  `earliest`/`overlapping` search kinds, `line-terminator`) that
  rurex's API cannot express.
