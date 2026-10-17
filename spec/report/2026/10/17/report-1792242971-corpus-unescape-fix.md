## Report on task: corpus unescape decoder fix (closes "bug 2")

Ad-hoc owner request: diagnose and fix the last ledger follow-up row,
`bytes/invalid-utf8-anchor-200` (bytes-mode empty match one byte off
on invalid UTF-8).

### Done

- Root cause: **harness defect, not a rurex defect.** The corpus's
  `unescape = true` haystacks use four escape forms — `\xNN`, `\\`,
  `\n`, `\r` (verified by scanning all 1,203 tests); rurex's driver
  decoded only `\xNN`, so `X\\\x0f` produced a 23-byte haystack where
  upstream's harness feeds 22 — every later offset (and the `$`-empty
  match) shifted by one. rurex answered correctly for the input it
  was given.
- Fix: `tests/rustregex/corpus.py::unescape_haystack` now decodes
  `\\`, `\n`, `\r`, `\t` alongside `\xNN` (unknown escapes pass
  through, as before).
- Validation: the corpus's own expectations act as the decoder's test
  — `invalid-utf8-anchor-200` passes (follow-up rows: 1 → 0), its
  `-100`/`-300` siblings and the `\n`/`\r` haystack tests stay green,
  both corpus suites 2,406 green, pytest 279 green.
- Ledger: 790 → 789 rows; the `follow-up` bucket is now empty (only
  its legend line remains).

### Spec/ADR amendments

- [acted] EXPECTED.md regenerated; report-1792240544 bug-2 marker
  updated in this commit.

### Future-task notes

- [open] docs.rs and raw.githubusercontent.com were Cloudflare-blocked
  during diagnosis (could not read regex-test's upstream decoder);
  the supported escape set was derived from the corpus itself and is
  provably complete for it. If the corpus is ever re-vendored with
  new escape forms, extend the decoder in the same pass.

### Tooling/process

- When a differential harness disagrees with its oracle by a constant
  offset, suspect the harness's input decoding before the engine —
  one `len(haystack)` print settled a two-day-old mystery in seconds.
