# PyPI distribution name: rurex

Date: 2026-10-15
Status: accepted

## Context

The package needs a distribution name that is free on PyPI (and, for
consistency, on crates.io). The natural short name `rex` is registered on
PyPI by a dormant project, as were most first-round candidates
(`rematch`, `rexpy`, `pyrexx`, `pyregex`, `regx`, `regex-rs`,
`rust-regex`, ...). Names one edit away from the top package `regex`
also risk PyPI's typosquat registration blocks. Free candidates were
evaluated on brand, grep-ability, and distance from existing projects:
`rexen`, `rustregex`, `faregex`, `linear-regex`, `rurex`.

## Decision

The distribution, import module, and Rust crate are named **rurex** — a
nod to `rure`, the Rust regex C API, extended for Python. It was free on
both PyPI and crates.io at decision time and is distinctive enough to
avoid confusion with stdlib `re` and the `regex`/`pyre2` package
families.

The name was registered on PyPI immediately: placeholder release 0.0.1
(pure-Python, setuptools-built, no extension code) was uploaded on
2026-10-15 — https://pypi.org/project/rurex/0.0.1/. The first
maturin-built release (ADR-0001) supersedes the placeholder. The
repository directory remains `rex`; only the published names are
`rurex`.

## Alternatives rejected

- **rex / pyregex / rematch / rexpy / ...** — already registered on PyPI
  by other, mostly dormant, projects.
- **rexen** — strongest brand fallback; kept in reserve.
- **rustregex** — self-describing but generic; reads as a category, not
  a package.
- **faregex / linear-regex** — lead with the linear-time guarantee, but
  less memorable.
- **Near-misses of `regex`** (`rgex`, `xregex`, `regexpr`) — risk
  rejection by PyPI's similarity protections at upload time.
