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

Run: `maturin develop && pytest` (the extension must be built into the
running environment first).

- Files: `tests/test_*.py`.
- Cover every public name in `spec/docs/api.md`: compilation, flags,
  search/match/fullmatch/is_match, findall/finditer, split/sub/subn,
  Match accessors, `rurex.error` on bad patterns, and `str`/`bytes` modes.
- Where `re` and `rurex` agree on semantics, compare outputs against the
  `re` module in the test to lock in compatibility.
- Where they differ (no backreferences/lookaround), assert the documented
  failure mode (`rurex.error`).

## Final checks (per session, before committing)

1. `cargo fmt --check` and `cargo clippy -- -D warnings` — clean.
2. `cargo test` — pass.
3. `maturin develop && pytest` — pass.
4. `python -c "import rurex"` — imports cleanly in a fresh interpreter.
5. No hard-coded credentials anywhere in the change.

CI runs the same checks on every push and PR to `main`
(`.github/workflows/ci.yml`): the `rust` job covers 1–3, the `python`
job builds the extension with `maturin develop` and runs `pytest`
across Python 3.10–3.15. Runs and logs:
https://github.com/lorien/rurex/actions
