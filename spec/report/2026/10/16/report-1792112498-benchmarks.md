## Report on task: Benchmark rurex vs re, publish results page

Ad-hoc work (owner-directed; plan approved after one round).

### Done

- `benchmarks/cases.py`: 16 cases across six regimes — 10 MB scans
  (literal/rare-word/anchored, targets at the worst-case end),
  1 MB match-heavy (`findall`/`finditer`/`sub`), call overhead
  (precompiled + module-cache dispatch), unicode and bytes, compile
  (simple/url/datetime), plus the pathological `(x+x+)+y` series.
  All haystacks deterministic (fixed seeds).
- `benchmarks/run.py`: timeit-style auto-calibrated iterations,
  median of 5 repeats, end-to-end timing (Python-visible cost incl.
  object construction), environment capture, JSON output. Two
  fairness fixes discovered during development: `re.compile`
  measures a cache hit unless `re.purge()`ed first (now done), and
  bytes patterns needed a JSON-safe repr.
- `benchmarks/make_charts.py`: seven styled matplotlib charts
  (grouped log-scale bars per regime with per-case verdicts, sorted
  speedup summary with parity line, pathological exponential-vs-flat
  line chart with timeout annotation).
- `docs/benchmarks.md`: end-user page — leads with the pathological
  chart (framed as safety, not speed), summary chart, methodology
  (release-build requirement, timing protocol, fairness notes),
  environment, per-regime tables + charts, interpretation (scan-bound
  vs match-dense vs compile-amortized), reproduction commands.
- README gains a Benchmarks link.
- Results on i5-8350U / CPython 3.13.5: rurex ×1.6–14.4 on scans;
  re ×1.5–2.3 on match-heavy extraction and ×5–8 on compile; rurex
  ×1.5 on small precompiled calls, ×1.2 behind on module-cache
  dispatch; pathological: re doubles per 2 chars (1.0 s at n=24,
  timeout at n=26) vs rurex flat ~256 ns.

### Spec/ADR amendments

- No spec semantics touched; README link added (the "no speed claims"
  stance now resolves to "measured claims, both directions"). [acted]

### Future-task notes

- [open] Charts were generated blind (model cannot render PNGs);
  owner reviewing visuals — label placement/color tweaks are
  one-line changes in make_charts.py followed by a re-run.
- [acted] Match-heavy regime is the known gap (per-match Python-level
  slicing in Match construction); a future optimization pass (direct
  Rust-side string slicing, batched span conversion) could close it —
  would change benchmark outcomes and require re-measuring this page.
  — Resolved 2026-10-16 by that pass: `findall` now scans in one
  GIL-released Rust loop and slices directly; dense-extraction Tests
  3/4 flipped to rurex ahead (×1.4 at 80/KB; parity at 160/KB); page
  re-measured.

### Tooling/process

- `re.compile` is cache-backed: benchmarking "compilation" without
  `re.purge()` per call reports ~300 ns cache hits. Any future
  compile benchmark must neutralize the cache on both sides
  (`rurex.compile` is uncached by design; module functions are the
  cached path and are benchmarked as such).
- `signal.alarm`-guarded timing works well for capping exponential
  cases; series stops at the first timeout and the chart annotates it.
