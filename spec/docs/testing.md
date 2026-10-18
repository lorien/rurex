# Testing

Two test layers: Rust unit tests next to the code, and a Python pytest
suite that exercises the module the way users do.

## Rust unit tests

Run: `cargo test`

- Live in `#[cfg(test)]` modules in `src/`.
- Cover flag translation, error mapping, and any pure-Rust helpers (e.g.
  backslash-substitution for `sub`).
- Must not require a Python interpreter.

## Python test suite

Run: `make test` (or `make build && pytest`; the extension must be
built into the running environment first). `make build` installs the
release build by default; `make build-debug` is available for a debug
artifact.

- Files: `tests/test_*.py`.
- Cover every public name in `spec/docs/api.md`: compilation, flags,
  search/match/fullmatch/is_match, findall/finditer, split/sub/subn,
  Match accessors, `rurex.error` on bad patterns, and `str`/`bytes` modes.
- Where `re` and `rurex` agree on semantics, compare outputs against the
  `re` module in the test to lock in compatibility.
- Where they differ (no backreferences/lookaround), assert the documented
  failure mode (`rurex.error`).

## Final checks (per session, before committing)

1. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` —
   clean.
2. `cargo test` — pass.
3. `make test-all` — pass. It builds the extension in release mode, runs
   the pytest suite, and runs each conformance suite in its own process
   (see "Oracle guard" below). The release build is intentional: it
   leaves the installed extension benchmark-ready.
4. `python -c "import rurex"` — imports cleanly in a fresh interpreter.
5. No hard-coded credentials anywhere in the change.

`make check` runs items 1–3 in order.

CI runs the same checks on every push and PR to `main`
(`.github/workflows/ci.yml`): the `rust` job covers 1–3, the `python`
job builds the extension with `maturin develop` and runs `pytest`
across Python 3.10–3.15. Runs and logs:
https://github.com/lorien/rurex/actions

## Oracle guard: never share a process with the `re` shim

`tests/cpython/conftest.py` replaces `sys.modules["re"]` with rurex
for the rest of the process. The parity layers (Layer B, Mode L) use
stdlib `re` as their oracle, so in a shared process they would
compare rurex with itself and pass vacuously — green, and no longer
testing anything. Run the CPython harness and the oracle-based suites
as separate pytest invocations; `make test-conformance` does exactly
that, one process per line.

`tests/re_guard.py` fails loudly if `re` is not the module CPython
ships (provenance via `__spec__`/`__file__`, plus CPython-only
attributes), independent of what replaced it. It runs at conftest
import (`tests/conftest.py`) and, authoritatively, where the oracle is
bound (`tests/rustregex/parity.py`, `tests/divergences/red_live.py`):
the shim is installed during collection, after the root conftest has
run, so the binding site is the check that catches a mixed
invocation. `tests/test_re_guard.py` covers the detector against
non-rurex fakes.

## CPython conformance harness

Run:

    .venv/bin/python -m pytest tests/cpython/vendored_test_re.py -q

CPython's own `test_re.py` (plus its `re_tests.py` data), vendored
verbatim from the `v3.13.5` tag with a provenance header (ADR-0009).
`tests/cpython/conftest.py` installs a shim so `import re` resolves
to rurex (`__name__="re"`, `Pattern`/`Match`/`Scanner`, module
functions, `_compile`) and stubs the `test.support` helpers the file
imports. Not collected by a plain `pytest` run — invoke explicitly,
and never in the same pytest process as an oracle-based suite (see
"Oracle guard" above).

Every test either passes or is listed in the `EXPECTED` ledger in the
conftest with a bucketed reason: by-design (citing ADR/api.md),
CPython internals, harness artifacts, or a named follow-up task
(error-message parity, buffer-protocol haystacks). Closing a gap
means removing the ledger entry in the same commit. When bumping the
vendored tag, re-triage and refresh the ledger.

## rust-regex corpus conformance (two layers)

Run:

    .venv/bin/python -m pytest \
      tests/rustregex/vendored_corpus_engine.py \
      tests/rustregex/vendored_corpus_parity.py -q

rust-lang/regex's own `testdata/` corpus (1,203 TOML cases), vendored
verbatim at the tag matching the `Cargo.lock` regex version — see
`tests/rustregex/PROVENANCE.md` and ADR-0010. Not collected by a
plain `pytest` run. Two layers:

- **A — binding conformance** (`vendored_corpus_engine.py`): rurex's
  Python-visible results vs the corpus's expected byte-offset spans;
  exercises the glue upstream CI never runs (flag translation,
  anchored/VERBOSE pattern wraps, `pos`/`endpos` windows, byte→char
  span conversion, iteration advancement, str/bytes duality).
- **B — parity** (`vendored_corpus_parity.py`): the same inputs
  through rurex and stdlib `re`; both must agree on the compile
  verdict and the full match iteration. A disagreement is a bug or a
  ledger row.

Every non-passing case is a row in `tests/rustregex/EXPECTED.md`
(bucketed: unmapped / by-design python-parity / by-design
engine-semantics / by-design rurex-superset / corpus-ahead-of-pinned
-crate / follow-up); the harness enforces exhaustiveness in both
directions. Regenerate the ledger with
`python tests/rustregex/gen_expected.py` after driver changes or
corpus re-vendoring, then re-review the diff. When `Cargo.lock`'s
regex version bumps, re-vendor the corpus at the matching tag and
re-triage.

## regex-engine-divergences corpus (two modes)

Run:

    .venv/bin/python -m pytest \
      tests/divergences/vendored_divergences_live.py \
      tests/divergences/vendored_divergences_engine.py -q

539 curated constructs from
`Kristofp/regex-engine-divergences` (MIT, pinned snapshot — see
`tests/divergences/PROVENANCE.md`), each chosen because real engines
disagree on it. Not collected by a plain `pytest` run. The tests are
**inputs, not oracles** (ADR-0011):

- **Mode L — live parity:** each construct through rurex and
  stdlib `re` on this interpreter (finditer; i/m/s/x flags). Hard
  comparison with EXPECTED-ledger discipline; a mismatch is a bug or
  a ledgered by-design family.
- **Mode E — engine contract:** rurex vs the upstream-recorded Rust
  answers (byte indices normalized to characters), as a *discussion*
  report — mismatches are classified as documented adjustments or
  version drift, never auto-failures; only unexplained `discuss`
  rows would require a ledger entry.

Regenerate the ledger with `python tests/divergences/gen_expected.py`
after driver changes or re-vendoring, then re-review the diff.
