## Report on task: Needle-position threading benchmarks (Tests 8–9)

Ad-hoc owner request (not from plan.md): add two threading tests —
one on GIL Python, one on free-threaded Python — with a fixed 4
workers, sweeping the needle position "similar to Test 2".

### Done

- **Test 8 — `threads_pos_gil`** and **Test 9 — `threads_pos_free`**:
  Test 2's workload (literal needle, fixed 8 MB haystack) as a fixed
  pile of 1000 searches split across 4 barrier-started workers, the
  needle's position (25/50/75/100%) as the single variable. Y =
  wall-clock for the pile, same protocol as Tests 6–7. Byte-identical
  haystacks to Test 2 (`with_needle_at(8192*1024, needle, pct)`), so
  every point cross-checks against Test 2's per-op column.
- suite.py: `THREADS_POS_FIXED_KB/WORKERS/PILE` constants, two TESTS
  entries with a new `"threads": 4` test key. run.py: `measure_point`
  takes `threads=None`; `threaded_search` uses it when present, else
  the sweep value (Tests 6–7 unchanged, verified both paths).
  make_charts.py: two PRESENTATION hints (linear/linear); chart
  naming and the pile-wall Y label pick the new tests up generically.
- Design decisions, driven by arithmetic: pile 1000 not 8000 (`re` at
  100% costs 10.1 ms/op → 8000-op point ≈ 81 s; 1000 keeps the worst
  point ≈ 10 s); position 0% excluded (a ~160 ns op gives the pile
  ~0.2 ms of work — thread startup alone costs milliseconds, the
  point would time the harness); 8 MB doc exceeds this CPU's 6 MB L3,
  so 4 parallel scanners share memory bandwidth — documented in the
  methodology and both sections.
- Full 9-test run (~3.5 min, release build); all charts +
  `docs/benchmarks.html` regenerated; docs refreshed to this run:
  intro (stale "Five tests" → "Nine"; it already listed seven), two
  new sections after Test 7, methodology threading bullet extended to
  Tests 6–9, Tests 1–7 tables re-dated and refreshed (all within
  noise except Test 6's 8-worker contention point: 20.7 s vs 13.4 s —
  single aggregate measurements of a contention-heavy workload vary).
- Results: Test 8 — `re` pays its serialized per-op cost at every
  position (Test 2 column × 1000 + 5–20% contention); rurex ×10.7–18.0
  ahead but overlaps only 1.2–1.3× over its own serialized work
  (GIL re-acquisition for Match construction + shared memory
  bandwidth). Test 9 — both engines scale uniformly (re 3.1× at every
  position; rurex 3.2–3.8×), below ideal 4× partly via L3 overflow
  (Test 7's cache-resident 1 MB piles let re scale 3.7× at 4 workers,
  4.4× at 8); rurex keeps ×9.9–13.6 at every position.
- Final checks: cargo fmt --check, cargo clippy -D warnings, cargo
  test (20), maturin develop + pytest (191), fresh import — all pass.
  Restored the release build in .venv afterwards (check #3 installs a
  debug build over it — see Tooling).

### Spec/ADR amendments

- [acted] docs/benchmarks.md (intro, methodology, Tests 8–9 sections,
  refreshed tables) — done in this session.

### Future-task notes

- [open] Threaded findall (extraction under threads, where per-match
  GIL-held construction dominates — the Test 3/4 regime) is still the
  missing concurrency axis; flagged in report-1792195393 and unchanged
  by this work.
- [open] Threading points are single aggregate measurements; Test 6's
  8-worker `re` point moved 13.4 → 20.7 s between runs. A repeat-and-
  median pass over threaded points (3 walls per point) would tame
  contention variance at ~3× the threading-test cost.

### Tooling/process

- The final-checks step `maturin develop && pytest` silently replaces
  the release rurex in .venv with a debug build — poison for any
  later benchmark numbers in the same session. Fixed here by re-running
  `maturin develop --release` and verifying per-op time (~105 µs, not
  ~10× that); consider making the check profile-aware or always
  finishing sessions with the release rebuild.
