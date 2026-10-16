## Report on task: CI failure on main — root cause and fix

### Done

- Caught by the owner after several red runs I had missed: CI was
  failing on `python (3.14t)` (and earlier on both t-cells for a
  different reason) while I watched only the Release workflow after
  each push. Process rule adopted: check CI after EVERY push to main
  before calling work done.
- First bug (both t-cells): uv-created venvs ship without pip; the
  shared build step's `.venv/bin/pip` exited 127. Fixed with
  `uv venv --seed`.
- Second bug (3.14t only): `SystemError: init function of rurex
  returned uninitialized object` on plain import, with or without
  `-X gil=1`. Local reproduction impossible — identical stack
  (cpython-3.14.8+freethreaded, rustc 1.97.1 and 1.99.1, seeded and
  unseeded venvs) green on the dev box. Bisected ON CI with a
  temporary workflow_dispatch matrix: cache-less jobs green on four
  rustc versions → the delta was Swatinem/rust-cache. Its default
  key carries no matrix values, so the 3.14t cell restored a
  full-match `target/` built against a different interpreter; cargo
  reused the stale pyo3 build config and produced a broken .so.
  Fix: `key: py-${{ matrix.python-version }}`. CI now green on all
  10 jobs; debug workflow removed, probes dropped.
- Blast-radius check on the same bug class: release.yml used shared
  per-arch cache keys. The PUBLISHED 0.1.0 wheels were spot-checked
  from PyPI (cp314t wheel: import + match on 3.14.8t; abi3 wheel:
  full 191-test suite on 3.12) — all fine, likely parallel
  cold-cache ordering. release.yml keys made python-aware anyway.

### Future-task notes

- [open] CI has no per-cell wheel import smoke for release builds
  (release.yml builds but never imports); a cheap post-build
  `python -c "import rurex"` per cell would turn this failure class
  into a build failure instead of a published-broken-wheel risk.

### Tooling/process

- Watching the workflow you expect is not watching the ones you
  touched: every push triggers BOTH CI and (on tags) Release; both
  must be observed.
- When local reproduction is impossible, bisect on CI itself with a
  disposable workflow_dispatch matrix — four parallel cells found
  what five local hypotheses could not.
- Cache poisoning is an environment: "the same commands pass here"
  is not evidence when one side restores caches the other never had.
