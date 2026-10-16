## Report on task: Wheels Phase 3 — publish 0.1.0

### Done

- Tagged `v0.1.0` (at 3f9de1f) and pushed; the Release workflow ran
  on the tag and all 20 jobs succeeded, including `publish` via
  trusted publishing (OIDC) — no stored secrets.
- Verified on PyPI from the index itself: 0.1.0 is the latest
  version, 19 files exactly as designed — {x86_64, aarch64} ×
  {cp310..cp315, cp310-abi3, cp314t, cp315t} all `manylinux_2_28` +
  the 27-file allowlisted sdist.
- `pip install rurex==0.1.0` into a clean venv + smoke (compile,
  named groups, flags, findall): green. (A first unpinned install
  resolved to the 0.0.1 placeholder from a stale index cache —
  resolved on refresh; the index reports 0.1.0 as latest.)
- README: PyPI + Python-versions badges; installation section
  rewritten for the published state (wheels + source fallback).
- plan.md archived (no open tasks remain); the three publish-time
  markers (PyPI README, free-threaded CI, badges) marked acted.

### Future-task notes

- [open] Owner: delete the 0.0.1 placeholder release on pypi.org
  (project → Manage → Releases), after which the filename ban on
  rurex-0.0.1 files is permanent and harmless.

### Tooling/process

- Nothing new; the artifact-inspection discipline from Phase 2b paid
  off one last time — verification reads the PyPI JSON index, not
  assumptions.
