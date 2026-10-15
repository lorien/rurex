# Mirror the Python re API

Date: 2026-10-15
Status: accepted

## Context

The module could expose the Rust `regex` crate's own API shape
(`find_iter`, `captures`, `RegexBuilder` options) or adopt the vocabulary
Python users already know from the stdlib `re` module. The choice decides
the shape of every public name in `spec/docs/api.md`.

## Decision

Expose an API modeled on Python's `re` module: `rurex.compile`,
`Regex` with `search`/`match`/`fullmatch`/`findall`/`finditer`/`split`/
`sub`/`subn`, `Match` objects with `group`/`groups`/`groupdict`/`span`,
and `re`-style flag constants (`IGNORECASE`, `MULTILINE`, `DOTALL`,
`VERBOSE`). Rust `regex` semantics live underneath, and the documented
differences (no backreferences, no lookaround, always-Unicode `str`
matching, linear-time guarantee) are stated in `spec/docs/api.md`.

Deliberate deviations, recorded in the API doc: no module-level
convenience functions (`re.search(pattern, text)` style) and no pattern
cache — users compile once and reuse the `Regex` object, which matches
the underlying engine's strengths.

## Alternatives rejected

- **Expose the Rust API 1:1**: idiomatic for Rust users, but foreign to
  the Python audience; every caller would learn new names for identical
  concepts.
- **Wrap/subclass stdlib `re` types**: couples the module to CPython
  internals of `re` and offers no compatibility benefit, since the
  engines' feature sets differ.
