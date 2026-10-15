## Report on task: Add module-level convenience functions

Ad-hoc work (owner-directed; reverses ADR-0004 decision 1 via
ADR-0006).

### Done

- `src/lib.rs`: module-level functions `search`, `prefixmatch`,
  `fullmatch`, `findall`, `finditer`, `split(pattern, text,
  maxsplit=0, flags=0)`, `sub`/`subn(pattern, repl, text, count=0,
  flags=0)`, and `purge()` — signatures mirror `re` (with
  `prefixmatch` for `match`). Each delegates to the corresponding
  `Regex` method on a cached compile.
- Pattern cache mirroring CPython's `re._compile`: `Mutex` around a
  `HashMap<(bool, Vec<u8>, u32), Py<Regex>>` with a `VecDeque` for
  FIFO order; 512-entry cap, one-oldest eviction, raw flags in the
  key (so `(p, 0)` and `(p, U)` compile separately, like `re`).
  `rurex.compile` remains uncached.
- `tests/test_modulefuncs.py` (9 tests): differential comparison
  against `re` for all eight functions (flags argument incl. stdlib
  constants, zero-width, unicode, bytes, callable repl), identical
  error paths, and cache behavior via object identity — cache hit
  returns the same `Regex` object, `purge()` invalidates, and
  flooding past 512 entries evicts the oldest (first) pattern while
  recent ones stay cached.
- Surface guard grew the nine names. Suites: 72 pytest + 20 Rust
  green; fmt/clippy clean.

### Spec/ADR amendments

- ADR-0006 added (module functions + cache; supersedes ADR-0004
  decision 1); ADR-0004 Status line notes the supersession; api.md's
  "no module functions" paragraph replaced with the function list and
  cache semantics, and the corresponding deviations bullet removed;
  index.md catalogs ADR-0006. [acted]

### Future-task notes

- [open] Module functions dispatch to methods via Python
  `call_method`; if hot-loop overhead ever matters, refactor the
  `Regex` methods into inherent helpers taking `Py<Regex>` and call
  them directly.
- [open] The static cache holds `Py<Regex>` handles; on interpreter
  shutdown a static is never dropped, so cached objects leak until
  process exit (CPython's `re` has the same shape via module state).
  If subinterpreter support ever matters, move the cache into module
  state.

### Tooling/process

- `HashMap::new()` is not const; the cache static uses
  `LazyLock<Mutex<...>>`.
- `Py<T>` extraction can fail with `CastError`, not `PyErr`;
  `From<CastError> for PyErr` makes `?` handle it.
- Stdlib 3.13 deprecation noticed while writing differentials: `re`
  is moving `maxsplit`/`count`/`flags` to keyword-only; our
  positional-accepting signatures are a harmless superset, tests use
  keywords on the stdlib side.
