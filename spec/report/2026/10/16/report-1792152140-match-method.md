## Report on task: Ad-hoc — ship `match` alongside `prefixmatch`

### Done

- `Regex.match(text, pos=0, endpos=None)` and module-level
  `rurex.match(pattern, text, flags=0)` added, behavior-identical to
  `prefixmatch` (one shared `match_impl`; PyO3 `#[pyo3(name = "match")]`
  since `match` is a Rust keyword).
- Verified against reality before writing the ADR: Python 3.15.0
  (2026-10-09, PEP 790) ships BOTH names — `prefixmatch` preferred,
  `match` soft deprecated, no runtime warning. ADR-0004 decision 2 had
  read 3.15 as replacing `match` and excluded it; rurex now mirrors
  CPython exactly (recommended name `prefixmatch`, compatibility name
  `match`).
- ADR-0007 written; ADR-0004 status line notes the partial
  supersession; spec index, api.md, and README updated.
- Tests: differential matrix now exercises `match` against stdlib
  `re.match` (name-for-name on 3.13); `test_no_match_alias` replaced by
  `test_match_and_prefixmatch_both_ship`; `__all__` surface updated;
  module-function equivalence test added. 96 Python + 20 Rust tests,
  fmt/clippy clean.

### Spec/ADR amendments

- ADR-0007 (this commit); ADR-0004 status note; api.md module-function
  and method lists; README differences section and example.

### Future-task notes

- [open] CPython `test_re.py` conformance harness (planned next):
  `match` now resolves unshimmed; remaining shim surface is
  `Pattern`/`Match` type aliases and the module object itself.
- [open] Local interpreter is 3.13 (no `re.prefixmatch`); differential
  tests keep the prefixmatch→match mapping until CI runs 3.15, where
  the name can be compared directly.

### Tooling/process

- Facts about released Pythons must be checked against python.org docs
  (or a newer interpreter), not asserted from memory or the venv's
  version — the venv lags (3.13 while 3.15 is out).
