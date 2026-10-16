# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## CPython error-message and position parity

Status: new
Priority: 2

`checkPatternError` asserts exact CPython message text and position;
rurex currently reports the engine's own wording. Map the tested
messages in `validate_pattern`/error mapping: `unterminated
character set`, `nothing to repeat`, `bad escape (end of pattern)`,
`missing ), unterminated subpattern`, `missing >, unterminated
name`, `multiple repeat` (needs rejection of stacked quantifiers),
the unbalanced-parenthesis position, and inline-flag error texts.
Then drop the "error-message parity" ledger entries (the largest
bucket after buffers).

## Build and publish wheels

Status: new
Priority: -1

Deferred until the API stabilizes: maturin-built wheels and PyPI
publishing on tags. Hybrid strategy (ADR-0005): version-specific
wheels for each supported CPython minor plus one abi3-py310 fallback
wheel per platform. First real version is 0.1.0, superseding the
placeholder 0.0.1 that holds the PyPI name (ADR-0003).

References: ADR-0001, ADR-0003, ADR-0005
