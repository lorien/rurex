## Report on task: Ad-hoc — fix CI failures on Python 3.10–3.12

### Done

- Diagnosed from the failed CI runs (gh run view --log-failed): all
  65–66 failures per old version were `AttributeError: module 're'
  has no attribute 'PatternError'` raised by OUR tests — the parity
  tests caught the stdlib exception under its 3.13+ name. The
  extension itself built and matched on 3.10 (visible in the logs
  before the test crashed). 3.13/3.14/3.15 jobs were green.
- `tests/test_error_parity.py` rewritten to the recorded-table
  contract: EXPECTED/TEMPLATE_EXPECTED tables (message, position)
  generated from the 3.13 build and pinned in the file; rurex is
  compared against the tables on EVERY version; on 3.13+ the local
  stdlib is additionally compared live (guards the tables
  themselves). Stdlib exceptions caught as `re.error` (exists on all
  versions).
- `tests/test_module.py::test_patternerror_is_the_class_error_the_alias`:
  stdlib cross-check guarded with `hasattr` (pre-3.13 `re.error` is
  literally named `error`).
- api.md states the contract: error messages/positions follow
  Python 3.13+ `re` wording on every supported interpreter.
- One real pre-3.11 bug found by local validation: composite flag
  values (`rurex.I | rurex.M`) carry no computed `_name_` on the old
  enum machinery, so `RegexFlag.__repr__` fell back to
  `rurex.RegexFlag(10)`. The repr now decomposes the value into
  canonical members when `_name_` is absent — identical rendering on
  all versions.
- Validated locally with uv-provided interpreters before handing
  back: 3.10, 3.11, 3.12, 3.13, 3.15 — 191 passed each (3.14: same
  generation, CI-green already). fmt/clippy/cargo test clean.

### Spec/ADR amendments

- api.md Errors section: the version policy sentence.

### Future-task notes

- [open] The uv-venv loop used here (`uv venv --python X && VIRTUAL_ENV=... maturin develop && pytest`) is the cheap local pre-CI
  matrix check — worth a Makefile target or a dev.md note so the
  next version-sensitive change is validated before pushing, not
  after CI burns a run.
- [open] Canonical-version bump remains a queued task: re-vendor the
  conformance suite from v3.15.0 and re-triage (recorded tables are
  already proven identical on 3.13–3.15, so this is suite-refresh
  work only).

### Tooling/process

- Two lesson-reinforcing catches: (1) the harness itself referenced a
  version-gated API — write tests against names that exist across the
  whole support matrix; (2) "green locally" meant green on the dev
  box's 3.13 only — uv makes the full matrix a two-minute local
  check, use it before claiming version compatibility.
