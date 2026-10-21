## Report on task: Republish every benchmark page

Owner question: "benchmark results are up to date?". They were not: the
rebar page and the project-suite page carried wheels from before the
per-thread-engine and `Pattern.count` work, the frontend-HTML page was
from the 0.2.0 release, and the suite page's result tables turned out to
have been maintained by copy-paste, with at least two tests showing
another test's numbers. Ad-hoc work, no record archived from `plan.md`.

### Done

- **Rebar page republished** with a local wheel built from `a90c3a5`
  (per-thread engines + `Pattern.count`), full curated + dictionary set,
  provenance recorded in the Method row and in `results/run.json`:
  score ×2.58 against the fastest engine (44 benchmarks), `rust/regex`
  ×1.08, `python/re` ×9.81.
- **Project suite republished** (`make bench`) with both interpreters
  rebuilt from the same tree; `benchmarks/results.json` now records the
  current extension hashes
  (`71bba061…` for 3.13 GIL, `c446aa4c…` for 3.14t).
- **Frontend-HTML suite republished** (`make bench-html`), which had not
  been re-run since the 0.2.0 release.
- **Found and fixed a process bug: the suite page's tables were
  hand-copied.** `docs/benchmarks.md` had "Test 3 — Extraction" showing
  Test 1's numbers, Tests 6/7 sharing one table, and Tests 2/8/9 sharing
  another. Added `benchmarks/bench_tables.py`, which rewrites all nine
  tables in place from `results.json` (prose stays hand-written), with a
  `--check` mode that fails when the page is stale; `make bench` runs it,
  and `spec/docs/dev.md` documents it.
- **Refreshed the page's prose numbers** against the new run (ratios,
  the single-thread floor at position 0, the thread-pile gains) and the
  recorded `rurex` version in its environment table (0.1.0 → 0.2.0).
- **Made the two chart generators stamp the run date** instead of the
  hard-coded "2026-10-19" they had been printing since that date,
  including on pages whose data was newer.

### Results

Per-page state after this session:

| page | data from | notes |
|---|---|---|
| `rebar-benchmark.md` | `a90c3a5` wheel, today | ×2.58 overall; `count` and per-thread engines included |
| `benchmarks.md` | today's `make bench` | tables now generated, prose refreshed |
| `telegram-benchmark.md` | per-thread-engine build (today) | `count` and the iterate fix do not touch its rows |
| `homepage-benchmark.md` | today's `make bench-html` | first refresh since the 0.2.0 release |

The suite page's numbers moved little, as expected: this week's work is
per-call and free-threaded, while the suite's points are scan-bound.
Test 1 is ×33–42 (was ×33–43), Test 3 ×1.8–1.9, Test 6's pile 0.19 s →
0.040 s from one to eight workers, Test 9's lead ×27–36.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` "Benchmarks" documents
  `benchmarks/bench_tables.py`, its `--check` mode, and why the tables
  are generated rather than copied.
- [acted] `Makefile`: `bench` runs `bench_tables.py` before
  `make_charts.py`.

### Future-task notes

- [open] The suite page's prose still cites numbers by hand ("eases at
  16 MB (×36)"), so a republish requires reading the tables. A
  `bench_tables.py --check` in CI would catch a stale table but not a
  stale sentence; worth a pass to move the remaining numeric claims into
  the generated block, or to trim them.
- [open] `benchmarks/results.json` and the two real-world suites record
  no measurement date, which is why their pages had to hard-code one.
  Adding a `measured_at` field would let the generators derive it.
- [open] The rebar engine venv is populated by hand (`pip install` a
  local wheel); `rebar build -e` would reinstall the PyPI wheel. Worth a
  small script so the provenance in `run.json` cannot describe a wheel
  that is no longer installed.

### Tooling/process

- The `make bench` → `make_charts.py` split hides a trap:
  `make_charts.py` only *renders* the page (charts plus the HTML twin),
  so the markdown numbers had no generator at all. Ask "who writes this
  file" before assuming a `make` target refreshed it.
- A cross-build probe measurement is not a measurement: the same trivial
  PyO3 method cost 47 ns in one build of the probe extension and 86 ns
  in the next. Both variants must live in one binary.
- Republishing takes ~13 minutes on the host (rebar 10, suite 3) plus
  the frontend-HTML suite; all runs take `/tmp/core4.lock`, and a probe
  that hangs inside a lock holder blocks every later run — check
  `pgrep`/`/tmp/core4.lock` before blaming the harness.
