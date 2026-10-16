## Report on task: Refactor benchmarks to parameter sweeps

Ad-hoc work (owner-directed: three tests, per-test doc sections with
pattern + data example, variable-parameter charts, HTML render).

### Done

- `benchmarks/suite.py` replaces `cases.py`: three tests, each with
  description, pattern, data example, and one or two parameter sweeps
  (scan: size + needle position; extract: size + match density;
  pathological: input length with rurex-only extended range).
- `benchmarks/run.py` rewritten as a generic sweep harness (median of
  3, autorange, SIGALRM timeout guard, environment capture, JSON with
  per-point values and None for timeouts).
- `benchmarks/make_charts.py` rewritten: line charts (parameter on X,
  time on Y; log axes chosen per sweep), end-point annotations, dashed
  timeout continuation, plus `--html` mode rendering
  `docs/benchmarks.html` — markdown-converted with base64-embedded
  images, standalone in any browser.
- `docs/benchmarks.md` restructured: per-test sections with pattern,
  data example, both charts, value tables, and interpretation; old
  fixed-size bar-chart page and its seven PNGs removed.
- README benchmarks line links both md and HTML.

### Spec/ADR amendments

- Documentation-only change. [acted]

### Future-task notes

- [open] Charts still generated blind (model cannot render images);
  owner reviewing docs/benchmarks.html — the line-chart layout,
  annotation placement, and log-axis choices all need eyeballing.

### Tooling/process

- Sweep results are far more informative than fixed-size bars: the
  extract test's density sweep shows the exact crossover point
  (sparse: rurex ×4.7; dense: re ×2.2), and the scan position sweep
  exposed re's instant-fast-path at needle-position 0 versus rurex's
  ~0.7 ms capture-machinery floor — a fixed cost worth investigating
  in the match-heavy optimization pass.
