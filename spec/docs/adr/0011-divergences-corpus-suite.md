# Vendored regex-engine-divergences corpus: inputs, not oracles

Date: 2026-10-17
Status: accepted

## Context

`Kristofp/regex-engine-divergences` (MIT) publishes 539 curated
regex constructs — pattern, test text, flags, category — each chosen
because real-world engines disagree on it, plus answers recorded by
the authors on 16 real runtimes (CPython `re`, the Rust `regex`
crate, PCRE2, RE2, Java, .NET, ...). It is language-agnostic rule
data, exactly the shape of a third-party "test suite" that is really
a set of tests to check rather than a source of truth.

## Decision

1. Vendor `constructs.json` plus the recorded `python.json` and
   `rust.json` answers (pinned snapshot commit, `PROVENANCE.md`),
   under `tests/divergences/`.
2. **The constructs are inputs; the oracles are live.**
   - **Mode L (hard):** rurex vs stdlib `re` on this interpreter,
     replicating upstream's recorded operation (finditer over the
     whole text; `i`/`m`/`s`/`x` flag mapping, other letters
     ignored). Mismatches are ledger-enforced (bug or named
     by-design family).
   - **Mode E (discussion):** rurex vs the recorded Rust answers,
     with recorded byte indices normalized to character indices.
     Never an auto-failure: mismatches are classified as documented
     adjustments (python-parity semantics, superset syntax) or
     upstream version drift; only unexplained `discuss` rows need a
     ledger entry.
3. Recorded third-party answers are never compared for pass/fail;
   drift between them and the live engines is upstream noise, noted
   in triage only.

## Consequences

- A third curated corpus pinning the parity boundary from yet
  another angle: 539 disagreement-focused constructs, checked
  against two live oracles. First-run triage: 208 mode-L mismatches,
  every one in a documented family (re-only syntax the engine lacks,
  engine superset syntax, class-set/nested-class parses, possessive
  quantifiers, `$`, word boundaries); zero unexplained mode-E rows.
- The corpus upstream is a living snapshot without tags: re-vendor
  deliberately (pin the commit in `PROVENANCE.md`) and re-run
  `gen_expected.py`.
