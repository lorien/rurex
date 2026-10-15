# Architecture

## Components

The repository is a single Rust crate that produces one CPython extension
module, `rex`.

    regex (crates.io)          the engine
        |
    src/regex.rs  Regex        Python-visible type; owns a compiled program
        |                     (regex::Regex for str, regex::bytes::Regex for bytes)
    src/match_.rs  Match       span/group view over one match
    src/flags.rs               flag bit constants -> RegexBuilder options
    src/error.rs               regex::Error -> rex.error (ValueError subclass)
        |
    src/lib.rs  #[pymodule]    module init, module-level functions (compile, escape)

## Compilation path

`rex.compile(pattern, flags=0)` validates inputs in Python types, maps flag
bits to `regex::RegexBuilder` options in `src/flags.rs`, builds the
compiled regex once, and stores it inside the `Regex` pyclass as a Rust
struct field. A compile failure is converted in `src/error.rs` to a raised
`rex.error` carrying the regex crate's message. Compilation cost is paid
once; matching methods reuse the stored program.

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
`rex.error`.

## Memory and safety

No `unsafe` code is expected in this crate (PyO3 and `regex` handle their
own internals). If `unsafe` is ever proposed, it needs an ADR. Wrapper
types own their data; nothing borrowed from Python outlives a call except
through PyO3's checked lifetimes.
