## Report on task: Makefile release-build defaults and targets

Ad-hoc owner request: "Fix Makefile", following the debug-build trap
found during the Telegram extraction work: `make test-all` ran a plain
`maturin develop`, leaving a debug extension installed, and the next
benchmark run measured that debug artifact by accident.

### Done

- `make build` is now an alias for the release build. The project is
  benchmark-driven; a debug extension left installed silently makes
  every measurement several times slower. `make build-debug` keeps the
  old day-to-day behavior available.
- `make test-all` depends on `build-release`, so the final checks leave
  the installed extension benchmark-ready. `make test` alone still
  assumes an already-built extension, as before.
- Added `make build-free` (release build in `.venv-t` when that venv
  exists; a missing free-threaded venv is not an error).
- Added `make fmt`, `make clippy`, `make rust-test`, and `make check`
  matching the commands in `testing.md`; `check` runs all of them,
  including `test-all`.
- Added `make bench` (main suite plus charts) and `make bench-telegram`
  (1M-post suite plus charts); both depend on `build-release`, and the
  Telegram target also builds `.venv-t` when present.
- All maturin invocations now set `VIRTUAL_ENV` explicitly, so maturin
  cannot install into the wrong interpreter when both venvs exist.
- Updated `spec/docs/dev.md`, `spec/docs/testing.md`, and the README
  development snippet to describe the new targets and the release
  default.

### Verification

- `make -n build build-free test-all bench bench-telegram` shows the
  expected commands.
- `make build`, `make build-free`, `make test` (295 passed),
  `make fmt`, `make clippy`, and `make check` all pass.
- `make check` ends with a release build installed in `.venv`.

### Future-task notes

- [open] Add a `make test-free` target that runs the pytest suite under
  `.venv-t`; the Telegram benchmark already exercises that interpreter
  but the ordinary test suite does not run there by default.
- [open] `make bench` and `make bench-telegram` regenerate committed
  result files. Decide whether benchmark targets should write a scratch
  copy first and require an explicit `PROMOTE=1` (or similar) before
  replacing the committed snapshots.
