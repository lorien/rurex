# `match` ships alongside `prefixmatch`

Date: 2026-10-16
Status: accepted (supersedes the "no `match` alias" half of ADR-0004
decision 2)

## Context

ADR-0004 decision 2 shipped `prefixmatch` as the only name for the
anchored-at-start method, reading Python 3.15 as replacing `match`.
Python 3.15.0 (released 2026-10-09, PEP 790) instead ships **both**
names: `re.prefixmatch()` / `Pattern.prefixmatch()` are added as the
preferred, more explicit name, while `re.match()` / `Pattern.match()`
are soft deprecated (no runtime warning, documented preference) and
retained for compatibility. Running CPython's `test_re.py` against
rurex — planned as a conformance probe — also requires `match`, since
the suite calls it throughout.

## Decision

Mirror CPython 3.15 exactly:

1. `Regex.match(text, pos=0, endpos=None)` and
   `rurex.match(pattern, text, flags=0)` ship with behavior identical
   to `prefixmatch` (one shared implementation).
2. `prefixmatch` remains the documented, recommended name; `match` is
   the compatibility name. No `DeprecationWarning` is emitted —
   soft deprecation, exactly as in CPython.
3. No other names are affected.

## Consequences

- Code written against `re` on any Python version runs unchanged —
  both the historic and the 3.15 spellings resolve.
- The CPython test-suite conformance harness needs no shim alias for
  `match`.
- ADR-0004 decision 2's "no `match` alias" clause is superseded; its
  `prefixmatch`-as-primary-name intent stands, now as a preference
  rather than an exclusion.
