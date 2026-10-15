# Conventions

## Documents

- One focused topic per document under `spec/docs/`; `index.md` catalogs
  them; `plan.md` is the open-task list.
- Docs describe the intended design, never known breakage. Fix the code,
  not the truth.
- Keep documents in sync with the implementation at every commit (see the
  Documentation Sync rule in `spec/skills/work.md`).

## ADRs

Decisions with real alternatives get a numbered record in
`spec/docs/adr/` at decision time. Format and workflow:
`spec/docs/adr/0000-adr-format.md` and `spec/skills/adr_tracking.md`.
Records are never deleted or renumbered; supersession is noted on the
`Status:` line. Plan records cite relevant ADRs on their `References:`
line; topical docs carry the current truth, ADRs carry the history.

## Rust

- Edition and toolchain versions follow `rust-toolchain.toml` /
  `Cargo.toml`; build with stable Rust.
- `cargo fmt` formatting is mandatory; `cargo clippy -- -D warnings` must
  pass.
- Public items get `///` doc comments; binding code never `unwrap()`s on
  Python-controlled input — errors go through `PyResult`.
- One module per source file as laid out in `spec/docs/overview.md`;
  unit tests live in `#[cfg(test)]` modules next to the code.

## Python (tests and packaging)

- Test files are `tests/test_*.py`, run by pytest; style: stdlib only,
  function-based tests, no fixtures unless truly shared.
- The package exposes only what `spec/docs/api.md` lists; no extra public
  names without updating that document first.

## Commits

Short imperative subject line (e.g. "Add flag mapping to RegexBuilder"),
one logical change per commit.
