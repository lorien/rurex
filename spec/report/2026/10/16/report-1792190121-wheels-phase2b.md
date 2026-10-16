## Report on task: Wheels Phase 2b — dry-run the release pipeline

### Done

- Three CI iterations to a fully green `workflow_dispatch` dry run
  (publish correctly skipped):
  1. All 18 cells failed: the `rust-manylinux` container tags do not
     exist ("manifest unknown"); sdist job (no container) passed.
  2. Native x86_64 green in `quay.io/pypa/manylinux_2_28_x86_64`; the
     rust-cross container variant failed — its env sets a linker for
     the HOST target that does not exist in the image, breaking every
     build script. Switched aarch64 to GitHub's free arm64 runners.
  3. All 17 jobs green, but artifact inspection caught two defects:
     the exclude-style matrix had killed every abi3 cell (no fallback
     wheels), and bare-runner aarch64 wheels were tagged
     `manylinux_2_34` (the runner's glibc) instead of the decided
     2.28 floor. Fixed with include-only matrices (9 cells/arch) and
     the pypa `manylinux_2_28_aarch64` container on the arm runner.
- Final dry run: 19/19 jobs green; artifact listing verified — every
  wheel `manylinux_2_28` on both arches (cp310..cp315, cp310-abi3,
  cp314t/cp315t) + sdist. A CI-built cp313 wheel installed into a
  fresh venv and the full 191-test suite run against it: green.
- The Phase-2 report's "cells never executed" open item is resolved
  (marked acted).

### Future-task notes

- [open] Phase 3 unchanged: owner registers the PyPI trusted
  publisher, tag v0.1.0, delete the 0.0.1 release after publish,
  README note + badge flip.

### Tooling/process

- A green run is not a correct run: the third iteration was fully
  green while shipping no abi3 wheels and the wrong glibc tag on
  aarch64. The artifact-download-and-inspect step (filenames carry
  the tags) is what caught both; it stays mandatory before every
  publish.
- Matrix style lesson: exclude-lists of 8 entries to keep 1 are how
  the abi3 cells silently vanished — include-only entries from the
  start.
