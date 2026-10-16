# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Build and publish wheels

Status: new
Priority: -1

Deferred until the API stabilizes: maturin-built wheels and PyPI
publishing on tags. Hybrid strategy (ADR-0005): version-specific
wheels for each supported CPython minor plus one abi3-py310 fallback
wheel per platform. First real version is 0.1.0, superseding the
placeholder 0.0.1 that holds the PyPI name (ADR-0003).

References: ADR-0001, ADR-0003, ADR-0005
