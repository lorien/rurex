# Spec Index

Catalog of the documents under `spec/docs/`. Recommended reading order:

1. [overview.md](overview.md) — what rurex is, scope, repository layout.
2. [conventions.md](conventions.md) — document, code, and commit
   conventions; the ADR convention.
3. [architecture.md](architecture.md) — crate structure, compilation and
   matching paths, error handling, safety rules.
4. [api.md](api.md) — the Python-visible API contract and its
   differences from `re`.
5. [dev.md](dev.md) — prerequisites and the local build/run loop.
6. [testing.md](testing.md) — the two test layers and the per-session
   final checks.
7. [plan.md](plan.md) — open tasks.

## Decision records

- [adr/0000-adr-format.md](adr/0000-adr-format.md) — the ADR format
  itself.
- [adr/0001-pyo3-maturin-toolchain.md](adr/0001-pyo3-maturin-toolchain.md)
  — PyO3 for bindings, maturin for build/packaging.
- [adr/0002-mirror-python-re-api.md](adr/0002-mirror-python-re-api.md) —
  API modeled on Python's `re`, Rust `regex` semantics underneath.
- [adr/0003-pypi-name-rurex.md](adr/0003-pypi-name-rurex.md) —
  distribution/import/crate name `rurex`; name claimed on PyPI with
  placeholder 0.0.1.
- [adr/0004-v0-api-surface.md](adr/0004-v0-api-surface.md) — the six
  v0 API surface decisions: no module functions, `prefixmatch`,
  `pos`/`endpos`, flag set, minimal `RegexSet`, `error(Exception)`.
- [adr/0005-hybrid-wheel-strategy.md](adr/0005-hybrid-wheel-strategy.md) —
  version-specific wheels for speed plus an abi3-py310 fallback wheel
  for rolling-release resilience; speed-tuned release profile.

Workflow files (task/report/ADR lifecycle, session workflow) live in
`spec/skills/`.
