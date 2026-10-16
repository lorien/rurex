## Report on task: CI workflow

### Task (archived from plan.md)

## CI workflow

Status: done

Add a GitHub Actions workflow running `cargo fmt --check`,
`cargo clippy -- -D warnings`, `cargo test`, and `maturin develop` +
`pytest` on a small Python/Linux matrix, on every push and PR.

### Done

- `.github/workflows/ci.yml`: push + PR to `main`, concurrency group
  cancelling superseded runs. Job `rust` (fmt/clippy/cargo test with
  rustfmt+clippy components); job `python` over the full matrix the
  owner selected — Python 3.10–3.15 with `fail-fast: false` — running
  the exact dev-loop commands from `spec/docs/dev.md` (venv, maturin
  develop, pytest). `Swatinem/rust-cache@v2` on both jobs (auto-keyed
  per job+matrix); no pip cache (nothing to hash against).
- Matrix selection informed by release research: 3.10 went EOL
  2026-10-01, 3.15.0 released 2026-10-09 — supported minors are
  3.11–3.15, plus our still-declared 3.10 floor. Owner chose the
  full 3.10–3.15 span over bookends.
- YAML validated locally (parse + structural assertions on jobs,
  matrix, and the rust step list); the commands it runs are the same
  ones verified green in every prior session.

### Spec/ADR amendments

- testing.md final-checks section notes CI parity with the session
  checklist. [acted]
- plan.md task archived; only the deferred publish task remains. [acted]

### Future-task notes

- [acted] The 3.15 matrix entry is unverified against real runners:
  if PyO3 0.30.1 cannot build for 3.15 headers yet, trim the entry
  and re-add when supported (observe on the first CI run).
  (Verified on the maiden run 38821913820: all six Python versions
  including 3.15 passed.)
- [acted] Free-threaded entries (3.14t/3.15t) belong with the publish
  — resolved 2026-10-16: 3.14t/3.15t added to the CI matrix (Phase 2)
  and ship as release wheels.
  task's wheel-strategy decision, not CI; revisit there.

### Tooling/process

- The runner cannot be exercised locally; local verification was
  YAML structure plus the fact that every command in the workflow is
  part of the per-session green chain.
- `actions/setup-python` pip caching requires a requirements file to
  hash; inline installs of maturin/pytest are faster to leave
  uncached.
