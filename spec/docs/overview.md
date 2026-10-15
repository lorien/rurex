# Overview

## What rurex is

`rurex` is a Python extension module that binds the Rust [`regex`](https://crates.io/crates/regex)
crate to CPython. It exposes a fast, safe, linear-time regular expression
engine as a Python package: compile a pattern once, then match `str` (and
`bytes`) against it with an API modeled on Python's `re` module.

The binding is implemented in Rust with [PyO3](https://pyo3.rs) and built
and packaged with [maturin](https://www.maturin.rs) (see ADR-0001). The
Python-visible API mirrors `re`'s names where the engines agree, and Rust
`regex` semantics live underneath (see ADR-0002).

## Scope

In scope:

- Compiling patterns with `re`-style flags (`IGNORECASE`, `MULTILINE`,
  `DOTALL`, `VERBOSE`, `ASCII`, `SWAP_GREED`).
- Matching over `str` and `bytes`, including Unicode character classes
  (`\p{Greek}`) and named groups (`(?P<name>...)`).
- `search` / `prefixmatch` / `fullmatch` / `findall` / `finditer` /
  `split` / `sub` surface with `pos`/`endpos` windows, plus
  single-pass multi-pattern `RegexSet` membership matching.
- Clear `rurex.error` exceptions for pattern and engine errors.

Out of scope (the underlying engine does not support them and never will):

- Backreferences (`\1`) and lookaround (`(?=...)`, `(?!...)`).
- Recursion or arbitrary code execution in patterns.
- Being a drop-in replacement for `re` (module-level caching, `re.Scanner`,
  locale-dependent matching, etc. are not provided).

## Repository layout

The repository directory is `rex`; the published package, import name,
and Rust crate are `rurex` (ADR-0003). The name is registered on PyPI
by placeholder release 0.0.1.

    AGENTS.md            pointer for coding agents (read first)
    Cargo.toml           Rust crate manifest (cdylib: the extension module)
    pyproject.toml       Python packaging via the maturin backend
    src/
      lib.rs             #[pymodule] registration, module-level functions
      regex.rs           the `Regex` type wrapping regex::Regex / regex::bytes::Regex
      regexset.rs        the `RegexSet` type for multi-pattern membership
      match_.rs          the `Match` type returned by search/prefixmatch/finditer
      flags.rs           flag constants and translation to regex::RegexBuilder
      error.rs           regex::Error -> rurex.error mapping
    tests/               pytest suite (test_*.py)
    spec/
      docs/              design documents (source of truth; see index.md)
      skills/            agent workflow files (work.md and friends)
      report/            session reports

Rust unit tests live in `#[cfg(test)]` modules next to the code in `src/`;
the `tests/` directory at the repo root is the Python (pytest) suite.
