## Report on task: Claim the rurex name on PyPI

Ad-hoc work (user-directed; not from `spec/docs/plan.md`).

### Done

- Researched the Rust regex landscape and existing Python bindings
  (context for the naming decision, recorded in ADR-0003).
- Checked candidate names against `https://pypi.org/pypi/<name>/json`
  (404 = free): `rex`, `pyregex`, and 14 other candidates taken;
  `rexen`, `rurex`, `rustregex`, `faregex`, `linear-regex` free (also
  free on crates.io).
- Built a minimal pure-Python placeholder package `rurex` 0.0.1
  (setuptools backend, src layout, `twine check` passed) and uploaded
  sdist + wheel with the token from `~/.pypirc`. Live at
  https://pypi.org/project/rurex/0.0.1/. Placeholder source for
  provenance (built in `/tmp/rurex-placeholder`, not committed):

      [build-system]
      requires = ["setuptools>=77"]
      build-backend = "setuptools.build_meta"

      [project]
      name = "rurex"
      version = "0.0.1"
      description = "Python bindings to Rust's regex crate - placeholder release reserving the name; the Rust-backed package is under development"
      readme = "README.md"
      requires-python = ">=3.9"
      license = "MIT OR Apache-2.0"

      [tool.setuptools.packages.find]
      where = ["src"]

- Synced docs to the name: module/crate renamed to `rurex` in
  overview, architecture, api, dev, testing, plan, index, AGENTS.md,
  and ADR-0002; ADR-0003 added.

### Spec/ADR amendments

- ADR-0003 (distribution name `rurex`, placeholder claim) added. [acted]
- `spec/docs/plan.md`: scaffold task uses crate/package `rurex`; the
  deferred publish task now notes the placeholder 0.0.1 it supersedes.
  [acted]

### Future-task notes

- [acted] The public PyPI page carries the minimal placeholder README;
  — resolved 2026-10-16: 0.1.0 published; the PyPI page now renders the
  real README.
  the first real release (plan.md "Build and publish wheels") should
  ship the project README and proper metadata.

### Tooling/process

- The system Python is PEP 668-managed: `build` and `twine` are not
  installed and `/usr/local/bin/twine` is a stale entry script. A
  throwaway venv (`python3 -m venv`, `pip install build twine`) works.
  `maturin` is also absent and will be needed for the scaffold task.
- PyPI name availability via the JSON API is a pre-check only; final
  truth is at upload time. The project page went live immediately after
  upload, while the JSON API still 404ed for a few minutes
  (propagation lag).
