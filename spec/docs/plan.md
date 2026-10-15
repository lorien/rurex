# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Scaffold the PyO3 extension crate

Status: new
Priority: 5

Create the cargo crate `rurex` (crate-type cdylib) with PyO3 and `regex`
dependencies, `pyproject.toml` using the maturin backend, `src/lib.rs`
with a `#[pymodule]` exposing `__version__` and `rurex.error`, and a
minimal `cargo test` + pytest skeleton that imports the module after
`maturin develop`. Everything must build clean under `cargo fmt`,
`clippy -D warnings`, and pytest with no public API beyond
`__version__`, `escape`, and `error`.

References: ADR-0001, ADR-0002, ADR-0003

## Implement regex compilation and flag mapping

Status: new
Priority: 4

Implement `rurex.compile` and the `Regex` constructor: flag constants in
`src/flags.rs` translated to `regex::RegexBuilder` options, and
`src/error.rs` mapping `regex::Error` to `rurex.error` with the engine's
message. Unit tests for every flag and for representative bad patterns
(backreference, lookaround, unbalanced paren).

References: ADR-0002

## Implement Match objects and the matching API

Status: new
Priority: 3

Implement `src/match_.rs` (Match with `group`/`__getitem__`,
`groups`, `groupdict`, `start`/`end`/`span`, `expand`, `re`,
`string`, `pos`, `endpos`, `lastindex`, `lastgroup`, `__repr__`,
`None`/`-1` unmatched semantics) and the `Regex` methods: `search`,
`prefixmatch`, `fullmatch`, `is_match`, `findall`, `finditer`,
`split`, `sub`, `subn`, with `pos`/`endpos` windows on the five text
methods (code-point bounds translated to byte offsets), a lazy
`finditer`, `re`-style templates plus callable `repl`,
copy/deepcopy/pickle via `__reduce__`, and GIL release around scans
per `spec/docs/architecture.md`. `str` and `bytes` modes both
supported.

References: ADR-0002, ADR-0004

## Implement RegexSet

Status: new
Priority: 2

Implement `rurex.compile_set(patterns, flags=0)` returning a
`RegexSet` wrapping `regex::RegexSet`: `is_match(text) -> bool` and
`matches(text) -> list[bool]`, membership only, flags applied to all
patterns. Coverage for empty pattern list, result ordering, and
`str`/`bytes` modes.

References: ADR-0004

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

Deferred until the API stabilizes: maturin-generated wheels (abi3,
manylinux/macOS/Windows) and PyPI publishing on tags. The PyPI name
`rurex` is held by placeholder 0.0.1 (ADR-0003); the first
maturin-built release supersedes it.

References: ADR-0001, ADR-0003
