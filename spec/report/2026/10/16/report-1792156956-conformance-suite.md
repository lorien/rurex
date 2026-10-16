## Report on task: Ad-hoc — Scanner class + vendored CPython conformance suite

### Done

- Vendored CPython `test_re.py` + `re_tests.py` from tag `v3.13.5`
  (provenance headers, PSF-2.0) into `tests/cpython/`; `support_stub.py`
  covers the 12 `test.support` helpers; `conftest.py` installs the
  `re`→rurex shim (with `_compile`), seeds `test.support`/`test.re_tests`,
  and carries the EXPECTED ledger. Shim install is lazy
  (`pytest_pycollect_makemodule`) so a plain `pytest` run never sees a
  patched `re` — this leaked once (bad hook choice) and was caught by
  our own suite.
- `rurex.Scanner` (ADR-0009) mirrors `re.Scanner` from v3.13.5 exactly
  (compound pattern, `m.lastindex-1` action selection, callable actions
  as `action(scanner, token)`, None dropped, zero-width stops, rest from
  the last accepted match — a CPython quirk the differential tests
  forced out). `Pattern.scanner()` result renamed to `SRE_Scanner`
  (mirrors `_sre.SRE_Scanner`).
- Compatibility fixes that fell out of triage: `rurex.PatternError`
  alias; `compile(Pattern)` passthrough (flags rejected like `re`);
  CPython keyword names (`string`/`pos`/`endpos`/`maxsplit`/`repl`/
  `count`) across methods and module functions; `Pattern.flags` folds
  leading global inline flags; scoped `(?a:...)` rewritten to the
  engine's `(?-u:...)`; `Match.regs` legacy getter; Match
  `copy`/`deepcopy`; CPython `TypeError` texts ("expected string or
  bytes-like object, got 'int'"); template octal escapes produce
  characters (not U+FFFD) in `str` mode (differential-caught).
- Result: 62/165 vendored methods pass; the other 103 ledgered with
  bucketed reasons (unsupported syntax: lookaround/backrefs/
  conditionals/atomic/possessive; buffers; error-message parity;
  repr-by-decision; LOCALE/DEBUG/enum; engine limits; harness). Zero
  unaccounted failures. Own suite: 107 passed; Rust: 20; fmt/clippy
  clean.

### Spec/ADR amendments

- ADR-0009; api.md (Scanner, SRE_Scanner, PatternError, regs, new
  difference entries: verbose Unicode whitespace, stacked quantifiers,
  empty-loop captures, keyword names); testing.md harness section;
  plan.md gained two follow-up tasks (buffers, error parity).

### Future-task notes

- [open] plan.md "buffer-protocol haystacks" (priority 2) — largest
  fixable cluster (~15 methods).
- [open] plan.md "error-message and position parity" (priority 2).
- [open] Scoped `(?a:\W)`-style negated classes still hit the engine's
  UTF-8 safety check in str mode; noted in the ledger via
  test_scoped_flags — revisit only if a user hits it.
- [open] When bumping the vendored tag (e.g. 3.14/3.15), re-triage and
  refresh the ledger (testing.md documents this).

### Tooling/process

- Fetch CPython facts from the exact tag before mirroring — two
  behaviors (Scanner action signature, `(?x)`+`\x85`) were opposite of
  what reading about them suggested; differential tests caught both.
- pytest hook pitfall recorded in conftest: shim install must happen
  in `pytest_pycollect_makemodule`, not `pytest_collect_file` — the
  latter fires before the test-file name filter and leaks the patch
  into unrelated runs.
