# The compiled class is `Pattern`, not `Regex`

Date: 2026-10-16
Status: accepted

## Context

The compatibility push (mirroring `re` name-for-name; ADR-0007 added
`match`) made the compiled pattern object's class name the remaining
surface difference: `re.Pattern` vs `rurex.Regex`. Nothing has been
released (only a name-holding placeholder exists on PyPI), so there
are no external users to migrate — the choice is free. An alias
(`rurex.Regex = Pattern`) was considered and rejected: a second public
name costs surface, docs, and tests forever, and serves nobody.

## Decision

1. The Python-visible class is `Pattern` (`<class 'rurex.Pattern'>`,
   `type(rx).__name__ == 'Pattern'`). There is no `Regex` in the
   public surface. The Rust struct remains `Regex` — only the pyclass
   name changed.
2. `re.Pattern` semantics adopted in the same change: value equality
   (pattern text + flags, str/bytes distinguished) with a consistent
   `__hash__` (dict-key usable), weakref support, typing subscription
   (`Pattern[str]`, `Match[str]`), read-only `groupindex`
   (`mappingproxy`), and `scanner(text, pos, endpos)` with
   `SRE_Scanner`'s match/search semantics (a failed `match()` consumes
   the rejected match — verified against CPython).
3. One deliberate divergence kept: `rurex.Pattern(pattern, flags)`
   is directly constructible (`re.Pattern()` raises `TypeError`).
   `rurex.compile` remains the canonical entry point.
4. `repr` stays `rurex.compile('a+')` — self-identifying, never
   pretending to be `re`.

## Consequences

- `isinstance`, `type(...).__name__`, equality, hashing, weakrefs,
  and typing line up with `re.Pattern`; the CPython conformance
  harness has one less category of skips.
- `RegexSet` keeps its name (`re`'s counterpart is also `RegexSet`).
- api.md/README/tests swept in the same commit; no deprecation path
  needed (no released versions).
