## Report on task: Are the Telegram and homepage benchmarks current?

Owner question: "Telegram and homepage benchmark results are up to
date?". The homepage page was (measured from `e2fa2e3`, and only a
compile-time change landed since); the Telegram page was not — its data
came from the per-thread-engine commit but five commits had landed
since, and neither result file recorded which extension it measured, so
the question could not be answered from the artifacts. Re-measured both
and closed the provenance gap. Ad-hoc work, no record archived.

### Done

- **Gave the two real-world suites the provenance the main suite
  already had.** `extension_identity()` (path, size, mtime, sha256,
  interpreter, GIL flag) moved from `benchmarks/run.py` to
  `benchmarks/measurement.py`, and `telegram.py`/`frontend_html.py` now
  record `measured_at` plus `extension` (the interpreter they run in)
  and `extension_free` (asked of the free-threaded interpreter through
  a one-line child probe) in their `environment` block.
- **Made their pages take the date from the data.** The chart
  generators now render `environment.measured_at` in the "measured on"
  footer and fall back to today only when the file predates the field,
  so a regenerated page no longer re-dates old numbers (the bug fixed
  earlier today for these two pages).
- **Re-measured both suites from the current tree** (`cb9ef03` + the
  benchmark-script changes), on the benchmark host under
  `/tmp/core4.lock`: Telegram as the documented median of three merged
  runs (32 points), then the frontend-HTML suite (32 points).
- `spec/docs/dev.md` "Benchmarks" now says every result file carries a
  measurement timestamp and the per-interpreter extension hashes, and
  that the two real-world pages date themselves from it.

### Results

Both pages now name the measured binaries:

| page | measured at | GIL extension | free extension |
|---|---|---|---|
| `telegram-benchmark.md` | 2026-10-22 16:36 UTC | `4c8f9f18…` (3.13.5) | `175a013b…` (3.14.8t) |
| `homepage-benchmark.md` | 2026-10-22 16:44 UTC | `4c8f9f18…` (3.13.5) | `175a013b…` (3.14.8t) |

Telegram, free-threaded rows against the previous publication (which was
a *single* run, so part of the difference is noise reduction from
merging three): rurex bytes 0.177 → 0.149 s at two workers, 0.193 →
0.145 s at three, 0.197 → 0.167 s at four; rurex str 0.207 → 0.175 s at
three, 0.213 → 0.184 s at four. GIL rows moved by under 3%, and the
homepage suite by under 2% everywhere except one free-threaded
four-worker str point (+18.6%), which is the known single-cold-run noise
on that page. Nothing in either suite moved in a way that the code
changes since the last run would predict, which is the expected
outcome: what landed after those runs was compile-time diagnostics, one
new method and a window-bounds fix.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` "Benchmarks": provenance fields and the
  page dating rule, as above.
- [acted] No ADR: benchmark plumbing.

### Future-task notes

- [open] `merge_runs.py` keeps the first run's `environment` block, so
  the merged Telegram file's `measured_at` is the *first* run's
  timestamp. Fine for provenance (all three are the same build and
  within minutes), worth noting if runs are ever merged across days.
- [open] The two real-world pages still have no `--check` equivalent:
  the main suite page is guarded by `bench_tables.py --check`, but
  these two can only be verified by reading their timestamps against
  `git log`. A `bench_tables.py`-style freshness check would need to
  know which commit each page's extension hash corresponds to, which
  the files do not record (commit hash is not in the extension).

### Tooling/process

- The reason this question was hard to answer is the same one recorded
  in the last report: a page that names only "rurex 0.2.0" and a date
  cannot be tied to a binary. Recording the extension sha256 at
  measurement time makes "is it current?" a comparison instead of an
  investigation, and it cost one moved function and two dict entries.
- Re-running the Telegram suite as three merged runs (its documented
  protocol) rather than one is what makes its free-threaded rows stable
  enough to compare across publications; the previous page was a single
  run.
