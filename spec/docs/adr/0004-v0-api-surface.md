# v0 API surface decisions

Date: 2026-10-15
Status: accepted (revised 2026-10-15 — see Revision; decision 1
superseded by ADR-0006; decision 2's "no `match` alias" clause
superseded by ADR-0007)

## Context

ADR-0002 fixed the strategy — a `re`-modeled API over the Rust engine
— but left the concrete surface open. Six decisions were taken with
the owner on 2026-10-15, after surveying stdlib `re`, the `regex`
module, the RE2 bindings (google-re2, pyre2), rure-python, and the
regress bindings.

## Decision

1. No module-level convenience functions and no pattern cache;
   `rurex.compile` is the only entry point for single patterns.
2. `prefixmatch` ships instead of `match` (Python 3.15's name for the
   anchored-at-start method); no `match` alias. `search` and
   `fullmatch` ship as in `re`.
3. `pos`/`endpos` on all five text methods (`search`, `prefixmatch`,
   `fullmatch`, `findall`, `finditer`), with `^`/`\A` anchoring at
   `pos`; code-point bounds translated to byte offsets internally.
4. Flags: `I`, `M`, `S`, `X`, plus `ASCII` (the engine's non-Unicode
   mode) and `SWAP_GREED`. Unsupported `re` flags (`LOCALE`,
   `TEMPLATE`, `DEBUG`) and unknown bits raise `ValueError` naming
   the flag.
5. Minimal `RegexSet` in v0: `rurex.compile_set(patterns, flags=0)`
   with `is_match(text) -> bool` and `matches(text) -> list[bool]` —
   membership only, no offsets or captures.
6. `rurex.error` subclasses `Exception`, exactly like `re.error`.
   The full fidelity bundle ships: `Match.expand`, `re`-style
   replacement templates (`\1`, `\g<name>`, `\g<0>`), copy/deepcopy/
   pickle via `__reduce__`, `__repr__`s, `Match.__getitem__`, `None`
   for unmatched group accessors and `-1` for their spans.

The contract itself lives in `spec/docs/api.md`.

## Revision (2026-10-15)

During implementation planning of the compile task, the owner revised
three points after researching the engine's actual behavior:

1. Decision 4's flag set drops `SWAP_GREED`: swapped greediness is
   pattern syntax (`(?U)`), and a rurex-specific flag would have
   forced an extension bit namespace with collision risk against
   future stdlib flags. The accepted flags are exactly `re`'s bits
   (`I=2`, `M=8`, `S=16`, `U=32`, `X=64`, `A=256`), with `re`'s own
   `fix_flags` semantics and messages for `U` (no-op on `str`, error
   on `bytes`, incompatible with `A`).
2. `Regex.flags` and all flag handling mirror CPython exactly,
   including `UNICODE` being forced on for `str` patterns compiled
   without `ASCII`.
3. Decision 6's error surface is upgraded to full `re.error`
   attribute parity (`msg`, `pattern`, `pos`, `lineno`, `colno`) by
   pre-parsing patterns with `regex-syntax` (the engine's own parser)
   under the same flag configuration; engine-limit errors carry
   `None` positions.

## Alternatives rejected

- Module functions with a bounded LRU cache (`re`/`regex` model) or
  without a cache (rure/google-re2 model: silent per-call recompile
  cost).
- Shipping `match` (soft-deprecated in Python 3.15), aliasing both
  names, or shipping neither; one clear name won.
- `pos`/`endpos` on a subset of methods, or nowhere (the regexrs
  model).
- Flags: `ASCII` only, or `I/M/S/X` only; silently ignoring
  unsupported flags (the pyre2 fallback lesson).
- Deferring `RegexSet` to a later release, or never shipping it.
- `error` subclassing `ValueError`, or multi-inheriting with
  `re.error` (couples to stdlib internals and misleads handlers
  expecting a backtracking engine).
- Trimming the fidelity bundle (`expand`/pickle/reprs deferred).
