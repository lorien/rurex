## Report on task: Split benchmark tests to one variable each

Ad-hoc work (owner-directed: remedy (B) for the mixed-variable
critique — each test now has exactly one variable, fixed context in
the title).

### Done

- `benchmarks/suite.py` restructured into five single-variable tests:
  scan-size (needle at end), scan-position (fixed 8 MB), extract-size
  (fixed 80/KB), extract-density (fixed 1 MB), pathological. Each test
  carries its own description, pattern, data example, and exactly one
  sweep; titles state the fixed context.
- Chart titles now use the full test title;
  `make_charts.py` presentation map updated for the new test names.
- Suite re-run (fresh numbers); `docs/benchmarks.md` rewritten as
  five self-contained test sections with per-test data examples,
  charts, tables, and interpretation; intro TL;DR updated; HTML render
  regenerated (5 embedded charts). md image refs verified against
  files.

### Spec/ADR amendments

- Documentation-only change. [acted]

### Future-task notes

- [open] Charts still generated blind; owner reviewing
  docs/benchmarks.html (carried over).

### Tooling/process

- Test 2's position-0 point (re 166 ns vs rurex 744 µs) is stable
  across runs — the capture-machinery floor noted earlier reproduces.
