## Report on task: Threading benchmarks (Tests 6–7)

### Done

- Two new single-variable tests, owner-designed: **Test 6** (threads
  on GIL Python) and **Test 7** (threads on free-threaded 3.14t).
  Variable: workers 1/2/4/8; fixed pile of 8000 whole-haystack 1 MB
  searches; Y = wall-clock seconds for the whole pile (the owner's
  "multiply per-doc time by documents" metric, made native).
- Protocol: barrier-started workers, pile split evenly, per-point
  wall = max of per-thread walls; one aggregated measurement per
  point. Documented in the methodology section, with the
  cache-residency caveat for the 1 MB doc.
- run.py: `threaded_pile_point` + free-threaded orchestration —
  GIL tests in-process, Test 7 spawned via a `.venv-t`
  free-threaded interpreter (env override `RUREX_FREE_PYTHON`
  supported), merged into one results.json; environment now records
  cpu count and the free interpreter version.
- suite.py: two entries (op `threaded_search`, `interpreter` field,
  `pile`); make_charts.py: two linear charts, wall-time Y label;
  docs: full sections, intro bullets, reproduce block with the
  `.venv-t` prep, environment table updated (4C/8T, 3.14.8).
- Full 7-test run (~2.5 min) re-measured everything; Tests 1–5
  tables refreshed to this run (within noise of the previous).
- Results: Test 6 — `re` pinned at ~10 s (worsens past 2 workers:
  GIL serialization + contention), rurex 0.5–0.8 s (13–21× ahead;
  flattens because the GIL-held Match-construction part of each call
  dominates its fast scans). Test 7 — both scale ~4× at 8 workers;
  rurex keeps 12–15× at every point (0.22 s vs 2.7 s at 8).
- Owner-visible correction along the way: my first tables used
  numbers from a stale DEBUG build and a misleading per-thread
  metric; the owner called it out twice. Final metric and numbers
  are release-build, fixed-pile wall time, cross-checked against
  Test 1 per-op times.

### Spec/ADR amendments

- docs/benchmarks.md (two sections, methodology, environment,
  reproduce).

### Future-task notes

- [open] Threaded findall (scan + per-match Python-object
  construction under the GIL) would show where the GIL-held ceiling
  sits for extraction workloads; deferred — `search` keeps the
  threading story clean.
- [open] Test 7 pins 3.14t; when 3.15t wheels ship CI-side, the
  harness's `.venv-t` prep could move to 3.15t for freshness.

### Tooling/process

- Never put numbers from two build profiles in one table — a caveat
  under garbage data does not un-garbage it. The PyPI release wheels
  in /tmp venvs provided instant release-build numbers without
  rebuilding.
