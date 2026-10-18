# Local Development

## Prerequisites

- Rust stable toolchain (rustup).
- Python 3.10 or newer.
- A virtualenv with the dev tools:

      python -m venv .venv
      source .venv/bin/activate
      pip install maturin pytest

  or simply `make venv`, which creates `.venv` with the full dev set
  (maturin, pytest, matplotlib, markdown, py-spy).
- Missing a tool you need (profiler, debugger, utility)? Ask the
  owner to install it — see "Missing instruments" in
  `spec/skills/work.md`. Do not improvise substitutes.

## Build and run

The extension is built into the active environment with maturin:

    make build            # release build (default; benchmark-ready)
    make build-debug      # debug build, fine for day-to-day work
    make build-free      # optional release build in .venv-t

or directly:

    maturin develop --release
    maturin develop              # debug

`make build` is deliberately a release build. A debug extension left
installed silently makes every benchmark several times slower; use
`make build-debug` when debug assertions are what you want.

Smoke check:

    python -c "import rurex; print(rurex.__version__)"

## Checks before committing

    make check

`make check` runs `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `cargo test`, and `make test-all`. `make test-all` builds
the extension in release mode (so the installed artifact stays
benchmark-ready), runs `pytest tests/`, and runs the conformance
suites — each in its own process, because the CPython harness replaces
`re` process-wide (see `spec/docs/testing.md`).

## Benchmarks

    make bench            # main suite + charts
    make bench-telegram   # 1M-post Telegram suite + charts
    make bench-html       # frontend HTML href/src suite + charts

`make bench-telegram` and `make bench-html` also run `make build-free`
when `.venv-t` exists, because those benchmarks measure both the GIL
and free-threaded interpreters. All benchmark targets depend on
`build-release`.

See `spec/docs/testing.md` for what each of these covers.

## Repository and CI

- Remote: `git@github.com:lorien/rurex.git` (already configured as
  `origin`). Web: https://github.com/lorien/rurex
- Authentication: the `gh` CLI, account `lorien`; the token lives in
  the OS keyring (via `gh auth login`), never in the repo, docs, or
  environment.
- Pushing: `git push -u origin main`. Every push and PR to `main`
  runs CI (`.github/workflows/ci.yml`).
- Watching runs: `gh run watch` or
  https://github.com/lorien/rurex/actions

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
