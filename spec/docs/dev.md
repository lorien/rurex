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

    python -c "import rex; print(rex.__version__)"

## Checks before committing

    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
    maturin develop && pytest

See `spec/docs/testing.md` for what each of these covers.

## Project configuration notes

- `pyproject.toml` uses the `maturin` build backend; the wheel's Python
  requirement and abi3 version pin live there.
- Dependency versions are pinned in `Cargo.toml`; run `cargo update`
  deliberately, not in passing.
