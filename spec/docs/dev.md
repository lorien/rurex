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

**Run published benchmarks on the benchmark host, not on the laptop.**
The machine is part of the measurement: published numbers come from
`web@regexus:/web/benchmark/rurex` (13th Gen Intel Core i5-13500,
10 physical / 20 logical CPUs, idle, governor `performance`), and
laptop numbers are not comparable with them. Sync the working tree
(no commit for transport), build both venvs there, run the target, and
copy the result file and charts back:

    rsync -a --exclude .env --exclude .venv --exclude .venv-t \
        --exclude target --exclude var ./ web@regexus:/web/benchmark/rurex/
    ssh web@regexus 'cd /web/benchmark/rurex \
        && make build-release build-free && make bench'
    rsync -a web@regexus:/web/benchmark/rurex/benchmarks/results.json \
        benchmarks/results.json
    rsync -a web@regexus:/web/benchmark/rurex/docs/img/ docs/img/

`make bench` rebuilds both interpreters, so the free-threaded tests
cannot silently run against a stale `.venv-t`. `results.json` records
the CPU and each interpreter's extension sha256, so every row is
traceable to a machine and a binary; if the environment line says a
laptop CPU, the run must not be published.

The real-world benchmarks pin the process to physical CPUs,
pin free-threaded workers one per core (GIL workers share the process
affinity), run a fixed CPU warm-up on a separate buffer before the
timed barrier, and record governor, load average and per-CPU frequency. Set
`RUREX_WARMUP=0` only for a smoke run; the published pages assume the
default 1 s warm-up. The measured corpus is never touched by the
warm-up.

Every benchmark runner holds `/tmp/core4.lock` for the whole matrix.
The lease is a host-local `flock`; the lock file is mode 0666 and carries
JSON owner metadata (user, PID, host, CPU list, start time, command). A
second run prints the owner, waits up to `RUREX_CORE_LOCK_TIMEOUT`
(default 600 s), and then hard-fails. Crash/exit releases the flock
automatically; stale metadata is harmless.

Each point also measures CPU efficiency: process CPU / wall for GIL
points, and the minimum worker CPU / wall for free-threaded points.
Points below `RUREX_MIN_EFFICIENCY` (default 0.90) or above
`RUREX_MAX_LOAD` are rejected and retried up to `RUREX_RETRIES`
(default 2). Repeated failure aborts instead of publishing a
contaminated number.

Non-Python runners use the same lease through the wrapper:

    python benchmarks/corelock.py -- cargo run --release --example bench

For A/B work use the paired runner rather than two separate benchmark
runs:

    python benchmarks/ab.py --bench homepage --threads 1,4 \
        --forms str,bytes --samples 6 \
        --baseline var/baseline-so --candidate var/candidate-so

`--aa` runs the same extension on both sides to calibrate the noise
floor. The runner alternates order, reports the median paired ratio and
a bootstrap confidence interval, and uses the same affinity/warm-up
protocol as the benchmark.

Published benchmark rows are the median of three complete runs. Run the
benchmark three times, copy each result file, then merge them:

    python benchmarks/telegram.py && cp benchmarks/telegram_results.json run1.json
    python benchmarks/telegram.py && cp benchmarks/telegram_results.json run2.json
    python benchmarks/telegram.py && cp benchmarks/telegram_results.json run3.json
    python benchmarks/merge_runs.py run1.json run2.json run3.json \
        -o benchmarks/telegram_results.json

Use `benchmarks/frontend_html.py` and `frontend_html_results.json` for
the homepage benchmark.

See `spec/docs/testing.md` for what each of these covers.

## Documents

`docs/` holds two Typst documents about the UCS to UTF-8 problem that
`src/utf8.rs` works around: a 14-page explainer (`ucs-explainer.typ`)
and a 9-slide deck (`ucs-slides.typ`). The compiled PDFs are committed
next to the sources, so reading them needs nothing installed.

    make docs-pdf

rebuilds both. It requires `typst`, a single static binary from
https://github.com/typst/typst/releases — not a Python or Cargo
dependency, and not needed for any other target. Regenerate the PDFs
whenever a `.typ` changes and commit the pair together.

Both sources pin `date` in `#set document`, so the output does not
depend on when it was built: rebuilding an unchanged source reproduces
the committed PDF byte for byte, and editing one `.typ` does not churn
the other document's file.

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
