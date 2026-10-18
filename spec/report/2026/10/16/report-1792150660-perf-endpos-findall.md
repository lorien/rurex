## Report on task: Ad-hoc — beat `re` where it was ahead (benchmarks Tests 2–4)

### Done

- Profiled the benchmark losses with py-spy (`--native`, release build
  rebuilt with `CARGO_PROFILE_RELEASE_DEBUG=2
  CARGO_PROFILE_RELEASE_STRIP=none` for symbols; codegen unchanged).
  Root cause of Test 2's ×4480 at needle@0 was **not** the engine:
  `Regex::search` called `s.chars().count()` on the whole haystack on
  every call to fill `Match.endpos` when `endpos=None`.
- `Match.endpos` is now lazy (`Endpos::Given | Eof(OnceLock)`,
  resolved and cached on first read; bytes mode resolves O(1) via
  `len`). All eager `chars().count()` sites removed from the matching
  paths (`search`, `fullmatch`, `finditer` construction,
  `substitute`'s callable branch, `prefixmatch`'s clamp).
- Capture names moved to `Arc<Vec<Option<String>>>` computed once at
  compile; per-match construction is now a refcount bump.
- `pos`/`endpos` window conversion (`window_bytes`) walks the text at
  most once, only as far as the indices actually given —
  `search(text)` with defaults walks nothing.
- `findall` rewritten as a dedicated path: one GIL-released Rust pass
  collects all match spans, then only the returned values are built
  (direct `&str`/`&[u8]` slicing; no `Match` objects, no per-match
  span→character conversion). `Match::piece`/`group_count` deleted.
- Re-measured the full suite (results.json, charts, HTML, and
  docs/benchmarks.md rewritten): Test 1 up to ×13.3 (was ×8.8);
  Test 2 needle@0 744 µs → 436 ns (re 170 ns), every other position
  ×7.6–12.9 rurex; Test 3 flipped from `re` ×1.3 to rurex ×1.4;
  Test 4 flipped from `re` ×2.0 to rurex ×1.05–6.2 across densities.
- New test: `endpos` default counts characters lazily on multibyte
  text. Architecture doc updated (findall fast path; the
  O-haystack-per-call prohibition). 95 Python + 20 Rust tests, fmt,
  clippy clean.

### Spec/ADR amendments

- `spec/docs/architecture.md` matching path updated (this commit).
  No ADR: no API or semantics change — `endpos` still reports the
  same values, just computed lazily.

### Future-task notes

- [open] Test 2 needle@0: `re` 170 ns vs rurex 436 ns. Both sub-µs;
  closing it would mean trimming per-call `Match` construction
  (~440 ns floor: pyclass alloc + `Py<Regex>` handle). Low value.
- [acted] Test 4 @160/KB is parity (×1.05). The remaining per-value
  cost is `PyString` construction across the FFI boundary; `re`
  builds values in C without one. Hard to beat without batching
  tricks that would complicate the code for ~5%.
  [acted — the flat findall path (find_at, one span buffer) removed
  the per-match Captures/Vec allocations; the 2026-10-18 suite re-run
  measures ×1.74 at 160/KB.]
- [open] `FindIter::step` still releases the GIL per match and builds
  a `Match` per match — by design (API returns Match objects), but a
  detach-only-when-haystack-remaining-is-large heuristic could shave
  dense `finditer` loops.

### Tooling/process

- py-spy (venv, `make venv`) resolved what an hour of source-diving
  could not; the venv was a phantom activation (moved from
  /web/rex; broken shebangs/activate) — recreated at the correct
  path via the new `make venv`.
- Debug symbols for profiling a release .so: maturin strips by
  default; `CARGO_PROFILE_RELEASE_DEBUG=2
  CARGO_PROFILE_RELEASE_STRIP=none` keeps symbols without changing
  codegen. Resolve py-spy raw addresses against
  `target/release/librurex.so` (installed .so is stripped) with the
  load base from `/proc/<pid>/maps` dumped by the target itself;
  `setarch -R` makes the base deterministic.
