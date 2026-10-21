# Spec Index

Catalog of the documents under `spec/docs/`. Recommended reading order:

1. [overview.md](overview.md) — what rurex is, scope, repository layout.
2. [conventions.md](conventions.md) — document, code, and commit
   conventions; the ADR convention.
3. [architecture.md](architecture.md) — crate structure, compilation and
   matching paths, error handling, safety rules.
4. [api.md](api.md) — the Python-visible API contract and its
   differences from `re`.
5. [dev.md](dev.md) — prerequisites and the local build/run loop.
6. [testing.md](testing.md) — the two test layers and the per-session
   final checks.
7. [plan.md](plan.md) — open tasks.

## Decision records

- [adr/0000-adr-format.md](adr/0000-adr-format.md) — the ADR format
  itself.
- [adr/0001-pyo3-maturin-toolchain.md](adr/0001-pyo3-maturin-toolchain.md)
  — PyO3 for bindings, maturin for build/packaging.
- [adr/0002-mirror-python-re-api.md](adr/0002-mirror-python-re-api.md) —
  API modeled on Python's `re`, Rust `regex` semantics underneath.
- [adr/0003-pypi-name-rurex.md](adr/0003-pypi-name-rurex.md) —
  distribution/import/crate name `rurex`; name claimed on PyPI with
  placeholder 0.0.1.
- [adr/0004-v0-api-surface.md](adr/0004-v0-api-surface.md) — the six
  v0 API surface decisions: no module functions, `prefixmatch`,
  `pos`/`endpos`, flag set, minimal `RegexSet`, `error(Exception)`.
- [adr/0005-hybrid-wheel-strategy.md](adr/0005-hybrid-wheel-strategy.md) —
  version-specific wheels for speed plus an abi3-py310 fallback wheel
  for rolling-release resilience; speed-tuned release profile.
- [adr/0006-module-functions-and-cache.md](adr/0006-module-functions-and-cache.md) —
  `re`-style module-level functions backed by a bounded FIFO pattern
  cache; supersedes ADR-0004 decision 1.
- [adr/0007-match-method.md](adr/0007-match-method.md) —
  `match` ships alongside `prefixmatch` with identical behavior,
  mirroring Python 3.15's dual naming; supersedes ADR-0004
  decision 2's "no `match` alias" clause.
- [adr/0008-pattern-class-name.md](adr/0008-pattern-class-name.md) —
  the compiled class is `Pattern` (no `Regex` in the public surface),
  with `re.Pattern` value semantics: equality/hash, weakref, typing
  subscription, read-only `groupindex`, `scanner`.
- [adr/0009-scanner-and-conformance-suite.md](adr/0009-scanner-and-conformance-suite.md) —
  `rurex.Scanner` mirrors `re.Scanner`; CPython's `test_re.py`
  vendored (pinned v3.13.5) as a conformance harness with an
  EXPECTED ledger accounting for every test.
- [adr/0010-rustregex-corpus-suite.md](adr/0010-rustregex-corpus-suite.md) —
  rust-lang/regex's `testdata/` corpus vendored (pinned to the
  Cargo.lock regex version) and run in two layers: binding
  conformance vs the corpus's expected spans, and parity vs
  stdlib `re` on identical inputs; EXPECTED ledger as in ADR-0009.
- [adr/0011-divergences-corpus-suite.md](adr/0011-divergences-corpus-suite.md) —
  `regex-engine-divergences` constructs vendored as inputs (not
  oracles): hard parity vs live `re` (mode L) and a discussion-mode
  engine-contract report vs recorded Rust answers (mode E).
- [adr/0012-simd-transcoder-and-detected-layout.md](adr/0012-simd-transcoder-and-detected-layout.md) —
  hand-written AVX2 UCS→UTF-8 transcoder (no new dependency) plus
  runtime-detected, self-validated PEP 393 offsets instead of
  hard-coded ones.
- [adr/0013-cold-str-required-prefix-prefilter.md](adr/0013-cold-str-required-prefix-prefilter.md) —
  compile-time required-first-byte prefilter over the UCS storage skips
  the cold transcode when no match is possible.
- [adr/0014-gil-released-transcode.md](adr/0014-gil-released-transcode.md) —
  releases the GIL around the cold transcode for source buffers of at
  least 32 KB, so long-document encodes overlap across threads.
- [adr/0015-single-parse-compile.md](adr/0015-single-parse-compile.md) —
  compiles first and validates only on rejection, and derives the
  first-byte prefilter on the first cold transcode, so an accepted
  pattern is parsed once.
- [adr/0016-per-match-object-cost.md](adr/0016-per-match-object-cost.md) —
  inline spans for up to four groups, group text built with the direct
  `str`/`bytes` constructors, and a name table only for patterns that
  name a group; the remaining per-match gap is PyO3 call machinery.
- [adr/0017-thread-local-capture-locations.md](adr/0017-thread-local-capture-locations.md) —
  one `CaptureLocations` per pattern per thread, keyed by a never-reused
  pattern id, so `search`/`finditer`/`sub` steps and grouped `findall`
  matches stop allocating a `Captures` per call.
- [adr/0018-free-threaded-per-call-cost.md](adr/0018-free-threaded-per-call-cost.md) —
  per-thread engine clones were measured and reverted: the shared engine
  pool is not what stops the free-threaded rows from scaling, per-call
  Python reference traffic is.
- [adr/0020-pattern-count.md](adr/0020-pattern-count.md) —
  `Pattern.count`, the one API addition beyond `re`: a counting call
  that builds no result object, defined as the length of the same
  iteration.
- [adr/0019-per-thread-engines.md](adr/0019-per-thread-engines.md) —
  supersedes ADR-0018: a per-thread engine clone (own cache pool, shared
  program) is what makes short-haystack scans scale on free-threaded
  Python; ADR-0018's refutation rested on a probe that recreated threads
  per round.

Workflow files (task/report/ADR lifecycle, session workflow) live in
`spec/skills/`.
