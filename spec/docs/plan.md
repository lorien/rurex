# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Comprehensive Python test suite

Status: new
Priority: 2

Fill `tests/` with pytest coverage of every public name in
`spec/docs/api.md`: compilation, all flags, all matching methods
including `prefixmatch` and `pos`/`endpos` windows, `RegexSet`
membership, Match accessors, error cases, `str`/`bytes` modes, and
differential cases comparing against the stdlib `re` where
semantics are shared (including `\A` anchoring and zero-width-match
behavior).

References: ADR-0002, ADR-0004

## CI workflow

Status: new
Priority: 1

Add a GitHub Actions workflow running `cargo fmt --check`,
`cargo clippy -- -D warnings`, `cargo test`, and `maturin develop` +
`pytest` on a small Python/Linux matrix, on every push and PR.

## Build and publish wheels

Status: new
Priority: -1

Deferred until the API stabilizes: maturin-built wheels and PyPI
publishing on tags. Hybrid strategy (ADR-0005): version-specific
wheels for each supported CPython minor plus one abi3-py310 fallback
wheel per platform. First real version is 0.1.0, superseding the
placeholder 0.0.1 that holds the PyPI name (ADR-0003).

References: ADR-0001, ADR-0003, ADR-0005
