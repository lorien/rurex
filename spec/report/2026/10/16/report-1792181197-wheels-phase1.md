## Report on task: Build and publish wheels — Phase 1 (local builds)

### Done (Phase 1 scope: Linux, local verification, no publishing)

- **Version-specific wheel**: `rurex-0.1.0-cp313-cp313-
  manylinux_2_34_x86_64.whl` (8.4 MB) — installed into a FRESH 3.13
  venv, full 191-test suite green.
- **abi3 fallback wheel** (ADR-0005): `rurex-0.1.0-cp310-abi3-
  manylinux_2_34_x86_64.whl` (0.9 MB) — installed on uv-provided
  3.10 and 3.12 interpreters, 191 tests green on each. Note the size
  gap vs the native build (stable-ABI + no per-version specialization)
  — the hybrid strategy's fallback slot works as designed.
- **Free-threaded wheel** (owner decision: include): built against a
  uv-provided 3.14t interpreter (`cp314-cp314t`, 0.9 MB), GIL
  confirmed disabled at runtime, 191 tests green. Bonus smoke beyond
  scope: 8 threads hammering a shared Pattern concurrently
  (finditer + sub) returned identical correct results — no
  corruption; full thread-safety testing stays a Phase 2 item.
- **sdist**: `rurex-0.1.0.tar.gz` (0.7 MB) — contents verified
  (Cargo manifest + lock, src/, README, LICENSE), pip-installed from
  source in a fresh venv with the local toolchain, import + match
  smoke green.
- Dev venv left on the editable debug build; suite re-run green.

Decisions recorded for Phase 2/3 (owner): Linux-first platforms,
free-threaded wheels included, **trusted publishing (OIDC)** for
PyPI auth — one-time registration on pypi.org on the owner side.

### Spec/ADR amendments

- None this phase (ADR-0005 already covers the strategy; plan.md task
  stays open for Phases 2–3).

### Future-task notes

- [open] sdist is fat (87 files: spec/, benchmarks/, .github/ ride
  along — maturin packages the whole git tree). Phase 2 should add
  `[tool.maturin] exclude` to ship a lean source dist.
- [open] Phase 2: `release.yml` on `v*` tags — manylinux (+musl?)
  matrix × 3.10–3.15 + abi3 + t-builds, trusted publishing; macOS /
  Windows wheels can be a follow-up per the Linux-first decision.
- [open] Thread-safety test suite for the free-threaded wheel
  (Phase 2, replaces today's smoke).

### Tooling/process

- The uv matrix loop (build → install wheel → pytest in a fresh
  venv) caught nothing this time but proved it: verification against
  installed artifacts (not editable dev builds) is the right
  pre-publish gate.
