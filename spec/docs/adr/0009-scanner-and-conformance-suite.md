# `Scanner` class and the vendored CPython conformance suite

Date: 2026-10-16
Status: accepted

## Context

The compatibility push (ADR-0007 `match`, ADR-0008 `Pattern`)
planned running CPython's own `test_re.py` against rurex as a
conformance probe. The suite imports `re.Scanner` and
`test.support`, and Debian ships neither. Two decisions were taken
with the owner:

1. Vendor the suite from the python repo (pinned tag `v3.13.5`,
   provenance header, PSF-2.0) instead of installing Debian's
   `python3-tests`-style package — reproducible, no sudo, pinned
   forever; the needed `test.support` surface is stubbed (~12 small
   helpers).
2. Add the missing `Scanner` class rather than skipping its tests.

## Decision

1. `rurex.Scanner(lexicon, flags=0)` mirrors `re.Scanner` from
   CPython 3.13 exactly: compound pattern `"(" + ")|(".join(...)`,
   actions selected by `m.lastindex - 1`, callables invoked as
   `action(scanner, token)`, `None` results dropped, zero-width
   stops the scan, `scan` returns `(results, rest)` with the rest
   computed from the last accepted match. Attributes `scanner`
   (compound `Pattern`), `lexicon`, `match`.
2. The `Pattern.scanner()` result class is Python-named
   `SRE_Scanner` (mirroring `_sre.SRE_Scanner`) so the public
   `Scanner` name belongs to the lexicon class.
3. The harness lives in `tests/cpython/`: `vendored_test_re.py` +
   `vendored_re_tests.py` (verbatim from the tag, header prepended),
   `support_stub.py` (test.support stand-in), `conftest.py` (installs
   `sys.modules['re']` = rurex-backed shim with `__name__='re'`,
   `Pattern`/`Match`/`Scanner`, `_compile`; seeds `test.support` and
   `test.re_tests`; applies the EXPECTED ledger). It is NOT collected
   by a plain `pytest` run — run explicitly (see testing.md).
4. Ledger rule: every test either passes or is in EXPECTED with a
   bucketed reason (`by-design` citing ADR/api.md, `internals`,
   `harness`, or `follow-up task`). Closing a gap removes the entry.

## Consequences

- 62 of 165 methods pass; 103 are ledgered (unsupported syntax
  clusters: lookaround/backrefs/conditionals/atomic/possessive;
  buffers for bytes patterns; error-message parity; repr-by-decision;
  LOCALE/DEBUG/enum flags; engine limits). The ledger is the honest
  compatibility statement and the worklist for follow-ups.
- Compatibility fixes that fell out during triage: `PatternError`,
  `Pattern.flags` folding of leading inline flags, scoped `(?a:)`
  groups (rewritten to `(?-u:)`), `compile(Pattern)` passthrough,
  `re` keyword parameter names (`string`, ...), `Match.regs`,
  `Match.__copy__/__deepcopy__`, CPython `TypeError` texts, and
  octal template escapes producing characters (not corrupt bytes)
  in `str` mode.
