## Report on task: Scaffold the PyO3 extension crate

### Task (archived from plan.md)

## Scaffold the PyO3 extension crate

Status: done

Create the cargo crate `rurex` (crate-type cdylib) with PyO3 and `regex`
dependencies, `pyproject.toml` using the maturin backend, `src/lib.rs`
with a `#[pymodule]` exposing `__version__` and `rurex.error`, and a
minimal `cargo test` + pytest skeleton that imports the module after
`maturin develop`. Everything must build clean under `cargo fmt`,
`clippy -D warnings`, and pytest with no public API beyond
`__version__`, `escape`, and `error`.

References: ADR-0001, ADR-0002, ADR-0003

### Done

- Created the crate: `Cargo.toml` (cdylib `rurex`, pyo3 0.30 +
  regex 1.13, edition 2024, rust-version 1.85, cargo feature `abi3`,
  speed-tuned release profile per ADR-0005), `rust-toolchain.toml`,
  `pyproject.toml` (maturin>=1.5, version 0.1.0, >=3.10), `README.md`,
  `.gitignore`.
- `src/lib.rs`: function-form `#[pymodule]` registering `__version__`
  (from `CARGO_PKG_VERSION`), `escape` (thin wrapper over
  `regex::escape`), and `error`; `#[cfg(test)]` unit tests for the
  escape path. `src/error.rs`: `create_exception!(rurex, Error,
  PyException)` per ADR-0004.
- `tests/test_module.py`: version format, escape behavior, error
  base class, and a public-surface guard asserting `rurex.__all__ ==
  ["__version__", "escape", "error"]` plus the non-underscore `dir()`
  set.
- Verified green: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test` (1 passed), `maturin develop && pytest` (4 passed), and
  the same suite against a real wheel install (`maturin build` +
  `pip install target/wheels/…manylinux…whl`, 4 passed). Built wheel:
  `rurex-0.1.0-cp313-cp313-manylinux_2_34_x86_64.whl`.
- Environment: created `.venv` (gitignored) with maturin 1.15 and
  pytest; rustfmt/clippy/libpython3.13 already present.

### Spec/ADR amendments

- ADR-0005 (hybrid wheel strategy + release profile + 0.1.0) added;
  recorded the wheel decisions from the scaffold discussion. [acted]
- plan.md: scaffold task removed (archived here); publish task now
  specifies the hybrid wheel matrix and 0.1.0. [acted]
- dev.md configuration notes updated for the hybrid strategy and
  maturin's package layout; overview.md layout gained README.md and
  rust-toolchain.toml; index.md catalogs ADR-0005. [acted]

### Future-task notes

- [open] CI and publish tasks must build the hybrid matrix:
  version-specific wheels per supported CPython minor plus one
  `--features abi3` fallback wheel per platform (ADR-0005).
- [open] `escape` currently accepts `str` only; the compile task
  should decide the bytes-mode escape story when `regex::bytes`
  enters (api.md documents bytes patterns as in-scope).

### Tooling/process

- maturin 1.15 always emits a package layout (`rurex/__init__.py`
  with `from .rurex import *`), even with no Python sources; PyO3
  maintains `__all__` on the extension module, which the shim uses —
  so `__version__` re-exports correctly and `rurex.__all__` is a
  reliable surface contract. Editable installs additionally leak the
  submodule name `rurex.rurex` into `dir()`; the surface test ignores
  exactly that artifact.
- clippy `-D warnings` rejects doc comments on macro invocations
  (`create_exception!`) — use `//` comments there.
- System Python is PEP 668-managed; the project venv at `.venv` is
  the dev environment (maturin, pytest installed).
