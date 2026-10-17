## Report on task: regex-engine-divergences corpus suite (modes L+E)

Ad-hoc owner request: integrate a third-party, language-agnostic set
of regex tests — the third conformance suite, on the owner's framing
that such a corpus is *inputs to check, not oracles*.

### Done

- Vendored `Kristofp/regex-engine-divergences` (MIT, commit
  `668aeff`, snapshot without tags): `constructs.json` (539 curated
  constructs chosen where real engines disagree), recorded
  `python.json`/`rust.json` answers, `PROVENANCE.md` with pin,
  license, and re-download command → `tests/divergences/`.
- Semantics pinned from upstream's `runners/run_python.py`: finditer
  over the whole text, i/m/s/x flag mapping only (other letters
  ignored), answers as (match text, start index) + error verdict.
- **Mode L — live parity (hard):** rurex vs stdlib `re` on this
  interpreter, per construct; EXPECTED-ledger discipline, enforced
  both directions. First run: 331 pass / 208 mismatch, every
  mismatch classified into documented families — re-only syntax the
  engine lacks (lookaround, backrefs, atomic/conditional groups,
  `\Z`, octal escapes), engine superset syntax (`\p`, `\Z`→no,
  `(?<name>`, `\z`, `(?-u)`, `(?R)`, `\b{start}`, anchor repetition,
  POSIX/nested/class-set operators, GNU `\<\>`), possessive
  quantifiers, `$`, word boundaries. **Zero unexplained rows.**
- **Mode E — engine contract (discussion, never auto-fail):** rurex
  vs the recorded Rust answers, respecting the corpus's per-flavor
  exclusions (338 of 539 constructs don't apply to the rust flavor)
  and normalizing recorded byte indices to character indices (their
  Rust runner reports bytes, their Python runner characters — found
  via one `(?-u)\b` construct shifting by one per multibyte char).
  Result: 199 pass, 2 documented adjustments, **0 discuss rows**.
- Wiring: `vendored_divergences_{live,engine}.py` (not collected by
  plain pytest; modules uniquely prefixed `red_*` to avoid pytest
  import collisions with the rustregex suite's `corpus.py`), CI added
  to the 3.13 conformance cell, `gen_expected.py` ledger generator,
  testing.md section, ADR-0011 + index entry.
- Checks: fmt/clippy/cargo test clean; pytest 279; CPython vendored
  suite green; all four conformance files together: 3,484 passed.

### Spec/ADR amendments

- [acted] ADR-0011, index.md, testing.md, EXPECTED.md — this session.

### Future-task notes

- [open] The corpus upstream is a living snapshot (no tags);
  re-vendor deliberately when it changes materially and re-run
  `gen_expected.py` (pin the new commit in PROVENANCE.md).
- [open] `mode E` currently normalizes only indices; if upstream ever
  records multiline/flags-sensitive answers differing from our
  pinned operation, revisit the runner-semantics note in
  PROVENANCE.md.

### Tooling/process

- Third-party recorded answers carry mixed index units across their
  own runners (bytes for Rust, chars for Python) — normalize before
  any cross-language comparison; one multibyte construct exposed it.
- Two pytest suites in sibling dirs without packages collide on
  module names (`corpus.py` each) — prefix modules uniquely per suite
  (`red_*`) so all conformance suites can run in one invocation.
