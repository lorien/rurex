## Report on task: Build and publish wheels — Phase 2 (release pipeline)

### Done

- **Sdist allowlist** (owner decision: with tests): Cargo
  `[package] include` — maturin honors cargo package semantics, so
  anything unlisted never ships (fail-closed; new repo files are
  excluded by default). Two glob subtleties caught empirically in
  var/sdist-probe before they shipped: (1) maturin's sdist walker
  includes even UNTRACKED files (the earlier "git-tracked only" claim
  was wrong; .gitignore was doing the work), and (2) bare-name
  patterns (`LICENSE`) match by basename at ANY depth — the first
  rurex attempt dragged 59 `.venv/**/LICENSE` files and
  `.pytest_cache/README.md` into the sdist. Slash-anchored patterns
  (`/LICENSE`) fix it. Final manifest verified exactly: 27 files
  (build essentials + src/7 + tests/14 incl. the vendored conformance
  harness), and the install gate re-run (fresh venv, pip install from
  tarball, 191 tests green).
- **release.yml** (tag `v*` + workflow_dispatch dry-run): one sdist
  job + an 18-cell wheel matrix in the manylinux_2_28 container
  (glibc floor = RHEL 8, owner decision) — {x86_64, aarch64-cross} ×
  {cp310..cp315, abi3-py310 fallback, cp314t/cp315t}; uv provisions
  interpreters including t-builds (the Phase-1-proven pipeline).
  Publish job: trusted publishing (OIDC, environment `pypi`, no
  stored secrets), artifact-count check (19), assets attached to the
  GitHub release; publishing active only on tags.
- **ci.yml**: `3.14t` and `3.15t` added to the regular test matrix
  via uv (closes the ci-workflow open note); the venv-creation step
  now reuses the uv-made t-venv.
- ADR-0005 revision records the resolved free-threaded question and
  the platform decisions; plan.md carries the Phase-3 checklist.
  Both workflow YAMLs parse; suite green locally (191).

### Spec/ADR amendments

- ADR-0005 Revision (2026-10-16); plan.md task body updated to the
  Phase-3 checklist.

### Future-task notes

- [open] Phase 3 (owner actions): register the PyPI trusted
  publisher, tag v0.1.0, delete the 0.0.1 release after 0.1.0 is
  live, flip the README note + badge.
- [acted] The 2_28/aarch64/cross cells of release.yml have never
  executed — first `workflow_dispatch` dry-run (no publish) is the
  verification step; run it before tagging v0.1.0.
  — resolved 2026-10-16 (commit 29ed34c): dry run green, 19 artifacts,
  every wheel tagged manylinux_2_28 on both arches, a CI-built wheel
  suite-tested locally. Two fixes were needed along the way (see the
  wheels-phase2b report).
- [open] Thread-hammer suite for t-builds still pending (smoke only).

### Tooling/process

- The include-glob basename trap is exactly the owner's blocklist
  critique in a second costume: allowlists also fail OPEN if their
  patterns over-match. The var/sdist-probe rig (git init + junk
  files + maturin sdist, no compiling) is the cheap way to test
  packaging semantics — keep it.
