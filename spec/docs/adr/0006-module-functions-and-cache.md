# Module-level convenience functions and pattern cache

Date: 2026-10-16
Status: accepted

## Context

ADR-0004 decision 1 shipped v0 without module-level match shortcuts
(`re.search(pattern, text)` style) and without a pattern cache, on the
grounds that compile-once is the engine's strength and that uncached
shortcuts quietly recompile per call (the rure/google-re2 disease). In
practice the owner wants `re`-symmetric ergonomics, and the original
objection was to the *uncached* form, not to shortcuts per se.

## Decision

Add module-level functions mir `re`'s surface: `search`,
`prefixmatch` (there is no `match` — ADR-0004 decision 2 stands),
`fullmatch`, `findall`, `finditer`, `split(pattern, text, maxsplit,
flags)`, `sub`/`subn(pattern, repl, text, count, flags)`, and
`purge()`. They delegate to the same `Regex` methods and share
`rurex.compile`'s build path, so every error (pattern `TypeError`,
flag `ValueError`, `rurex.error` with full attributes) is identical.

A bounded pattern cache backs them, mirroring CPython's
`re._compile`: keyed by `(pattern type, pattern text, flags)`; 512
entries maximum with FIFO eviction (the oldest entry is dropped when
the cap is reached, so one pattern recompiles rather than the whole
cache); `purge()` clears it. `rurex.compile` itself remains uncached,
keeping explicit compile-once semantics.

This supersedes ADR-0004 decision 1; the rest of ADR-0004 is
unchanged.

## Alternatives rejected

- No module functions at all (ADR-0004's original stance) — owner
  preference for `re` symmetry; the ergonomic cost was real.
- Module functions without a cache — rejected originally and still
  wrong: per-call recompilation punishes exactly the usage the
  shortcuts encourage.
- LRU eviction instead of FIFO — marginally better hit rates, but
  FIFO is what CPython does; no reason to diverge.
- Caching `rurex.compile` itself — explicit compilation is already
  cheap to reason about; mixing cache semantics into the explicit
  API would blur it.
