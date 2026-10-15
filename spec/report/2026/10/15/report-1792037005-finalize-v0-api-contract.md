## Report on task: Finalize the v0 API contract

Ad-hoc work (user-directed design discussion; not from
`spec/docs/plan.md`).

### Done

- Researched precedent designs feeding the decision: rure-python
  (byte offsets leaked through with warnings), crate-py/regress
  (code-point slices, correct by construction), Python-world `re`
  alternatives (`regex`, google-re2/pyre2, hyperscan), and the
  Python 3.15 soft-deprecation of `re.match` in favor of
  `re.prefixmatch`.
- Ran a six-question decision round with the owner, one question per
  message. Outcome (recorded in ADR-0004):
  1. No module-level convenience functions, no pattern cache.
  2. `prefixmatch` ships; no `match` alias; `fullmatch` stays.
  3. `pos`/`endpos` on all five text methods.
  4. Flags `I/M/S/X/ASCII/SWAP_GREED`; unsupported/unknown flags
     raise `ValueError`.
  5. Minimal `RegexSet` in v0 (`compile_set`, `is_match`, `matches`).
  6. `rurex.error` subclasses `Exception`; full fidelity bundle
     (expand, re-style templates, copy/pickle, reprs, `__getitem__`,
     `None`/`-1` unmatched semantics).
- Rewrote `spec/docs/api.md` as the decided contract, including the
  offsets rule (code points everywhere, conversion internal to Rust)
  and the deviations section (`\Z` vs `\z`, `$` at trailing newline,
  no `match`).
- Synced docs: ADR-0004 added; plan.md matching task rewritten, new
  `RegexSet` task, test-suite task extended; overview scope/layout
  and architecture diagram/compile path updated; index.md cataloged
  ADR-0004.

### Spec/ADR amendments

- ADR-0004 (v0 API surface) added. [acted]
- api.md contract rewritten to the decided surface. [acted]

### Future-task notes

- [open] The test-suite task must cover the decided semantics with
  differential cases against `re`: `\A`-anchoring at `pos`,
  zero-width-match iteration/substitution, and the `\z`/`$`
  deviation cases from api.md.

### Tooling/process

- Web research (rure-python, regress, regex/RE2/hyperscan families,
  gh-148100 prefixmatch) proved decisive for the API decisions;
  keeping research artifacts in conversation only was sufficient —
  no repo artifacts needed beyond ADR references.
