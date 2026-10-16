## Report on task: Accept buffer-protocol haystacks for bytes patterns

### Task (archived from plan.md)

## Accept buffer-protocol haystacks for bytes patterns

Status: new
Priority: 2

`re` accepts any bytes-like object (bytearray, memoryview, array,
bytes subclasses) wherever a bytes pattern scans; rurex accepts
exact `bytes` only. Largest conformance cluster after the by-design
skips (see the EXPECTED ledger in `tests/cpython/conftest.py`,
bucket "buffer types"). Accept the buffer protocol in `extract_hay`
(copy for one-shot methods; a held export for iterators if the
BufferError semantics of `test_keep_buffer` are worth mirroring),
plus bytes-like `repl`. Then re-triage and remove the ledger entries.

### Done

- `extract_hay` restructured: returns `HayKeep` (owns the source —
  original `bytes`/subclass borrowed zero-copy, or a `bytes` copy of
  a bytes-like input) with `hay()` deriving the borrowed view; all
  call sites in `regex.rs`/`regexset.rs` updated.
- Conversion via `builtins.bytes(obj)` after an authoritative
  buffer-protocol probe (`memoryview(obj)` succeeding) — the probe
  exists because `bytes(5)` silently yields NUL bytes (a Python
  quirk that let `int` haystacks through; caught by the new tests).
  `pyo3::buffer::PyBuffer` was avoided deliberately: it is empty
  under the crate's own `abi3` feature (found via `--all-features`).
- `finditer`/`scanner` snapshot a bytes-like input once at creation
  (`FindIter`/`SreScanner` gained a snapshot field); `Match.string`
  keeps the original object. Documented deviation: `re` locks the
  buffer (resize → `BufferError`); we snapshot (resize invisible).
- `Match.group()` on a bytes-like `string` slices a byte conversion
  — slicing the object itself is item-wise for `array` (a 2-item
  `array('h')` slice returned 4 bytes; caught by the new tests).
- Bytes-like `repl` accepted in `sub`/`subn` (copy) and callable
  returns converted likewise.
- Conformance: 4 methods fully green now (test_re_match,
  test_re_findall, test_empty_array, test_bug_29444 — 62→65 passing,
  103→100 ledgered); 4 buffer-ledgered methods re-ledgered with
  their true remaining reasons (lookaround subtests,
  DeprecationWarning, keep_buffer snapshot deviation). 15 new
  differential tests in `tests/test_buffers.py`; own suite 122
  passing; fmt/clippy/cargo test clean.

### Spec/ADR amendments

- api.md Bytes mode: bytes-like haystacks/repl, snapshot semantics,
  group() returning bytes. plan.md task removed (archived above).

### Future-task notes

- [acted] plan.md "error-message and position parity" (priority 2) is
  — resolved 2026-10-16 by commit 729da30 (error parity task).
  now the largest remaining fixable conformance bucket.
- [open] If `BufferError`-on-resize semantics ever matter, the
  snapshot fields in FindIter/SreScanner are the place to hold a
  real export instead (needs PyBuffer under a non-abi3 build).

### Tooling/process

- `--all-features` in clippy gates is what exposed the abi3/PyBuffer
  interaction — keep running final checks with it.
- Two real bugs (int-through-bytes(), item-wise array slicing) were
  both caught immediately by differential tests written alongside
  the feature — the pattern keeps paying off.
