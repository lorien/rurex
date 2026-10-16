# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Build and publish wheels

Status: new
Priority: -1

Deferred until the API stabilizes: maturin-built wheels and PyPI
publishing on tags. Phase 1 (local artifact verification) and Phase 2
(sdist allowlist, release.yml, CI t-matrix) done 2026-10-16; all six
release decisions recorded in the wheels reports and ADR-0005
revision. Remaining Phase 3: owner registers the PyPI trusted
publisher (project settings -> Publishing -> lorien/rurex, workflow
release.yml, environment pypi), push tag v0.1.0, verify the 19
artifacts on PyPI, delete the 0.0.1 placeholder release (after 0.1.0
is live), flip the README installation note + badge, archive this
task. Hybrid strategy (ADR-0005): version-specific
wheels for each supported CPython minor plus one abi3-py310 fallback
wheel per platform. First real version is 0.1.0, superseding the
placeholder 0.0.1 that holds the PyPI name (ADR-0003).

References: ADR-0001, ADR-0003, ADR-0005
