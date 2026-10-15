# Local Development

## Prerequisites

- Rust stable toolchain (rustup).
- Python 3.10 or newer.
- A virtualenv with the dev tools:

      python -m venv .venv
      source .venv/bin/activate
      pip install maturin pytest

## Build and run

The extension is built into the active environment with maturin:

    maturin develop              # debug build, fine for day-to-day work
    maturin develop --release    # optimized build, for benchmarks

Smoke check:

    python -c "import rurex; print(rurex.__version__)"

## Checks before committing

    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
    maturin develop && pytest

See `spec/docs/testing.md` for what each of these covers.

## Project configuration notes

- `pyproject.toml` uses the `maturin` build backend; the Python
  requirement (>=3.10) lives there. Wheel strategy is hybrid
  (ADR-0005): default builds are version-specific; the abi3-py310
  fallback wheel is built with `--features abi3`.
- maturin generates a package layout (`rurex/__init__.py` re-exporting
  the extension via `from .rurex import *`); PyO3 maintains `__all__`
  on the extension module, which drives the re-export. The submodule
  name `rurex.rurex` visible in editable installs is an artifact of
  this shim, not part of the API.
- Dependency versions are pinned in `Cargo.toml`; run `cargo update`
  deliberately, not in passing.
