# Architecture

## Components

The repository is a single Rust crate that produces one CPython extension
module, `rurex`.

    regex (crates.io)          the engine
        |
    src/regex.rs  Regex        Python-visible type; owns a compiled program
        |                     (regex::Regex for str, regex::bytes::Regex for bytes)
    src/regexset.rs  RegexSet  multi-pattern membership in one scan
    src/match_.rs  Match       span/group view over one match
    src/flags.rs               flag bit constants -> RegexBuilder options
    src/error.rs               regex::Error -> rurex.error (Exception subclass)
        |
    src/lib.rs  #[pymodule]    module init, module-level functions (compile, escape)

## Compilation path

`rurex.compile(pattern, flags=0)` validates inputs in Python types,
maps flag bits to `regex::RegexBuilder` options in `src/flags.rs`
(mirroring CPython's `fix_flags`, including its error messages and
the `UNICODE`-forced-on normalization for `str` patterns), and builds
the compiled regex once, storing it inside the `Regex` pyclass as a
Rust struct field. Bytes patterns are built with the engine's Unicode
mode off, so classes are ASCII-only single bytes exactly like `re`.
Before compilation the pattern is parsed with `regex-syntax` (the
engine's own parser, under the same flag configuration) so syntax
errors carry structured positions; `src/error.rs` turns them into
`rurex.error` instances with the full `re.error` attribute set
(`msg`, `pattern`, `pos`, `lineno`, `colno`). A compile failure at
the engine level (size limits) is mapped to `rurex.error` with `None`
positions. `rurex.compile_set` does the same for `regex::RegexSet`,
storing one program that answers membership for all patterns in a
single scan. Compilation cost is paid once; matching methods reuse the
stored program.

## Matching path

- `Regex` holds the compiled engine; it is immutable after construction.
- Text inputs are accepted as `str` (Unicode, `regex::Regex`) or `bytes`
  (`regex::bytes::Regex`). Which engine is used is fixed at compile time by
  the pattern's/flag's character mode, not per call.
- Matching calls release the GIL (`py.allow_threads`) around the scan: the
  borrowed text reference is obtained before the release and no Python
  objects are touched inside the closure. This keeps long scans from
  blocking other Python threads.
- Results are converted back to Python objects (`Match`, tuples, lists)
  after the scan, under the GIL.

## Error handling

All fallible paths return `PyResult`. Rust panics must not cross into
Python: panicking code paths are bugs to fix, not to map. `regex::Error`
is the only expected failure at the boundary and always maps to
`rurex.error`.

## Memory and safety

No `unsafe` code is expected in this crate (PyO3 and `regex` handle their
own internals). If `unsafe` is ever proposed, it needs an ADR. Wrapper
types own their data; nothing borrowed from Python outlives a call except
through PyO3's checked lifetimes.
