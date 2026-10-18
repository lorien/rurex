## Report on task: Guard oracle suites against a shadowed stdlib `re`

Ad-hoc work, not a `plan.md` task.

### Done

- `tests/cpython/conftest.py` swaps `sys.modules["re"]` for rurex for
  the whole process. The parity layers (rust-regex Layer B,
  divergences Mode L) use stdlib `re` as their oracle, so collecting
  them in the same pytest invocation compared rurex with itself and
  passed vacuously.
- Added `tests/re_guard.py`: rejects any module that is not CPython's
  stdlib `re`, independent of what replaced it. Provenance is
  `__spec__.origin` under `<stdlib>/re.py` (3.10) or
  `<stdlib>/re/__init__.py` (3.11+), with `__file__` agreement and the
  CPython markers `_MAXCACHE`, `_cache`, `purge` as a second line of
  defense.
- Guard runs at `tests/conftest.py` import (fast fail) and,
  authoritatively, at the oracle binding in `tests/rustregex/parity.py`
  and `tests/divergences/red_live.py`: the shim is installed during
  collection, after the root conftest has run, so only the binding site
  catches a mixed invocation.
- `tests/test_re_guard.py`: 9 detector tests, including non-rurex fakes
  (bare module, wrong name, forged `__file__`, forged spec outside the
  stdlib, forged provenance without CPython markers).
- `Makefile`: `build`, `test`, `test-conformance` (two pytest processes,
  with a comment on why they must stay separate), `test-all`.
- Docs: `spec/docs/testing.md` gains an "Oracle guard" section;
  `spec/docs/dev.md` final checks use `make test-all`.
- Verified: 288 main tests, 71 CPython conformance (94 skipped), 3484
  conformance cases; the mixed invocation now aborts with the
  RuntimeError (exit 2) instead of passing. `cargo fmt --check`,
  `cargo test` (37), `cargo clippy -- -D warnings` clean.

### Spec/ADR amendments

- [acted] `spec/docs/testing.md` documents the process-wide shim, the
  separate-process rule, and the guard; `spec/docs/dev.md` points at
  `make test-all`.
- [open] Consider an ADR for the invariant "oracle suites never share a
  process with the CPython shim"; testing.md is the only record today.

### Future-task notes

- [open] `.github/workflows/ci.yml` runs the rust-regex/divergences
  conformance step but not the CPython `test_re.py` harness, so that
  ledger is only exercised locally. Decide whether to add it (one
  interpreter version is enough).
- [open] A future test module that binds stdlib `re` as an oracle should
  call `require_stdlib_re()` next to the binding rather than rely on the
  root conftest, for the ordering reason above.

### Tooling/process

- [acted] `make test-conformance` keeps the two invocations separate and
  carries a load-bearing comment, since merging them is otherwise a
  natural "cleanup".
- [open] The `cargo` checks are not make targets; `make test-all` covers
  Python only. Consider a `check` target that runs fmt, clippy, and
  `cargo test`.
