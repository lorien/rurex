## Report on task: Five CPython-conformance tests from `var/test5.txt`

Owner handed over `var/test5.txt`, which claims five `test_re.py` tests
that unirex passes and rurex does not, with a proposed fix for each, and
asked to adopt what is correct and makes sense. Four were adopted; one
was verified as a real divergence but its proposed fix is not
implementable, and the ledger entry now says why instead of blaming
error-message parity. Ad-hoc work, no record archived from `plan.md`.

### Done

Every claim was checked against CPython 3.13.5 on this machine first,
then against rurex.

1. **`test_constants` — adopted (ledger entry removed).** The test only
   asserts flag aliases (`re.L == re.LOCALE`, `I`, `M`, `S`, `X`); it never
   compiles with `L`. Verified it passes with the entry gone, no failing
   subtests. The old reason ("LOCALE/L flag unsupported") was misleading.
2. **`test_empty_array` — adopted (ledger entry removed).** Passes with
   the entry gone; the message-parity gap that motivated it no longer
   affects this test.
3. **`test_dollar_matches_twice` — verified, not adopted.** The divergence
   is real (`rurex.sub('$','#','a\nb\n')` is `'a\nb\n#'`, `re`'s is
   `'a\nb#\n#'`; `re.search('$','a\n')` is at 1, rurex's at 2). It is
   already documented in `api.md` ("`$` matches at the end of the
   haystack only") and is not expressible: `re`'s non-MULTILINE `$` is
   "end, or just before a final newline", the engine has no such
   assertion (its `Look::EndLF` is the *multiline* one: before every
   newline) and no lookaround, and the consuming rewrite
   `(?:\n$\z|\z)`… would change spans and the iteration result. The
   alternatives — running the pattern twice (once against a
   newline-truncated haystack) and merging the two match streams, or
   post-adjusting spans — change the meaning of every other
   end-sensitive construct in the pattern and put `findall`/`sub`
   parity at risk. The ledger entry stays, with that reason.
4. **`test_re_groupref_exists_errors` — adopted.** rurex reported the
   engine's generic `unknown extension ?(` for every conditional. Added
   `flags::conditional_check`, a left-to-right scan in the style of the
   other textual passes (`group_name_check`, `multiple_repeat`): it skips
   escapes, classes and comment groups, validates the reference type and
   value (digits: `0` is `bad group number`, `>= _sre.MAXGROUPS` is
   `invalid group reference N`; names: identifier rules, then "opened
   earlier"), the reference's closure, the body's closure, and the
   two-branch limit, then checks numeric references against the pattern's
   final group count — CPython's order. All 14 assertions of the test now
   match, message and position.
5. **`test_re_groupref_overflow` — adopted by the same scan** (assertion
   (b), the `(?(1073741823))` pattern, now reports `invalid group
   reference 1073741823` at position 10; assertion (a), the template
   case, was already correct).

Porting: unirex's `prevalidate()`/`PreScan` is a full pre-parser (it also
re-derives flag and group-position rules). rurex already has those checks
as three small scans, and it rejects *all* conditionals anyway, so only
the conditional rules were needed; the scan was written in rurex's
existing style rather than importing the bigger machine.

Verification: `cargo test` (47, including 18 conditional cases and 7
"CPython accepts these, stay silent" cases), `make test` (449), and the
conformance suite — **75 passed / 90 skipped / 175 subtests** (was 71 /
94 / 159). The corpus and divergence layers are unchanged (3,484).

### Verification method

For claims 1 and 2 the ledger entry *was* the assertion, so the only way
to check them is to run the suite with the entry removed: the five
entries were deleted in a scratch tree, the five tests run, and only the
two that passed were kept deleted. The rest of the file's claims were
checked by compiling each pattern with `re` and with `rurex` and
comparing `(msg, pos)`; all 23 patterns (the test's 15, plus valid
conditionals and edge cases) now agree or differ only in the intended
way (valid conditionals still fail as unsupported).

### Spec/ADR amendments

- [acted] `spec/docs/api.md` "Semantic differences" says what an invalid
  conditional reports (`bad group number`, `bad character in group
  name`, `unknown group name`, `invalid group reference N`, more than two
  branches), and that a valid one still fails as unsupported.
- [acted] `spec/docs/architecture.md` "Compilation path" lists the
  conditional validation with the other textual passes, and says why it
  exists (the engine rejects every conditional, so the scan decides only
  *which* error is reported).
- [acted] `README.md`'s "not supported" line for conditionals mentions
  that invalid ones carry `re`'s message and position.
- [acted] No ADR: this extends the existing textual-pass mechanism; the
  interesting decision (do not chase `$`) is a non-change recorded in the
  ledger reason and here.

### Future-task notes

- [open] `$`-before-a-final-newline remains the one documented `$`
  divergence. If it is ever wanted, it needs engine support (a
  `before-final-newline` assertion) — a `regex-automata` change, not a
  binding rewrite.
- [open] The textual passes now number four (`inline_adjust`,
  `group_name_check`, `multiple_repeat`, `conditional_check`), each
  scanning the pattern separately, and the compile path's only parse is
  the engine's (ADR-0015). A pattern with several *different* errors
  reports whichever pass runs first, not the leftmost, as CPython does;
  merging them into one scan that reports the earliest offset would fix
  that, and would be the moment to consolidate.
- [open] `is_python_identifier` uses `char::is_alphabetic`/`is_alphanumeric`
  as an approximation of Python's `str.isidentifier` (XID). It rejects
  every case CPython rejects in this test, but a name that is XID-valid
  and not `char`-alphabetic (or the reverse) would diverge; no such case
  is known.

### Tooling/process

- "A ledger entry is the only record of a failure" means a wrong entry
  is invisible: `test_constants` and `test_empty_array` had been skipped
  for reasons that no longer applied (or never did), which is a
  maintenance cost the harness cannot surface. Running the suite once
  with the whole ledger stripped, and reviewing what actually passes, is
  a cheap periodic check.
- The owner-provided note was accurate on every factual claim it made
  (including the exact messages and positions), and wrong only in
  proposing a fix for something the engine cannot express — worth
  separating "the fact is right" from "the fix is implementable" when
  adopting suggestions.
