## Report on task: Reuse CaptureLocations instead of allocating Captures per call

### Task (archived from plan.md)

## Reuse `CaptureLocations` instead of allocating `Captures` per call

Status: done

Every `search`/`match`/`fullmatch`/`finditer` step calls
`regex::Regex::captures_at`, which allocates a `Captures` object and its
location vector per call; grouped `findall` does the same per match. That
is the largest remaining item in the text `search` gap to `re`: the
result-path report measured text `search` at ~3,500 instructions per call
on a short haystack, within ~14% of `re`, with this allocation a
substantial share of what is left.

`Regex::captures_read_at` writes into a caller-provided
`CaptureLocations`, so the fix is to keep one per engine per thread
(thread-local, keyed by a stable engine id so a freed engine's slot is
never reused) and copy the spans out of it. The engine's `Regex` is
already shared across threads, so the storage must be thread-local rather
than a field.

References: ADR-0006,
`spec/report/2026/10/18/report-1792364009-result-path-offsets.md`,
`spec/report/2026/10/18/report-1792325206-findall-prefilter-speedup.md`

### Done

- **Per-thread capture-location scratch** (`src/regex.rs`): a
  thread-local table of four `CaptureLocations` per mode (text and
  bytes), keyed by a pattern id from a global `AtomicU64` handed out at
  compile time. Ids are never reused, because keying by the pattern's
  address would let a freed pattern's slot be picked up by a new one at
  the same address, and `captures_read_at` requires the locations of the
  pattern that created them. Four slots so alternating patterns do not
  thrash, round-robin eviction so a thread's scratch is bounded.
- `captures_spans_text`/`_bytes` now fill the scratch with
  `captures_read_at`, copy the spans into `Spans`, and drop the borrow
  inside the call. The flat `findall` scan (`scan_all_flat_text`/`_bytes`)
  borrows the scratch once for the whole scan instead of allocating per
  match.
- `Regex` carries the id; `FindIter` and `SreScanner` carry a copy so
  their per-step closures do not re-borrow the pattern.
- **Found and fixed a pre-existing `endpos` bug** while verifying
  parity: `pos=0, endpos=0` returned matches over the whole text, because
  `window_bytes` skips its bounds walk when neither index is beyond 0 and
  therefore never picked up `endpos = 0`. Fixed in its own commit
  (cc95418) with a regression test for `search`/`finditer`/`findall`.
- Republished the rebar page from a full curated + dictionary run with
  the new wheel.

### Results

Per-call A/B on the benchmark host: a 470-character text, 300,000
`search` calls per round, five rounds per measurement, three alternating
rounds per wheel, leased core, pinned CPU. Before = wheel from commit
5425fab (sha256 8bba10b5…), after = this work tree (sha256 27e2ffee…).

| pattern | before | after | change |
|---|---|---|---|
| no capture groups | 118.5 / 118.8 / 119.8 ns | 99.6 / 99.9 / 100.6 ns | **−16%** |
| one group | 179.0 / 180.8 / 181.2 ns | 162.0 / 162.0 / 163.4 ns | −10% |
| two groups | 173.9 / 176.0 / 176.3 ns | 160.6 / 160.9 / 162.3 ns | −8% |

That is the whole per-call overhead of one allocation and one free, and
it applies to every site that used `captures_at`: `search`, `match`,
`fullmatch`, each `finditer` and scanner step, each `sub` step, and each
grouped `findall` match.

As a cross-check on a development machine (where `perf` is available and
the host's `perf_event_paranoid` is not), instruction counts for the same
400,000-call script fell from ~1.87e9 to ~1.84e9, i.e. about 75
instructions per call; `perf` is not installed on `regexus`, so the
published numbers are wall clock.

Republished curated rebar run (same protocol as previous editions):

| benchmark | before | after | change |
|---|---|---|---|
| `01-literal/sherlock-en` | 62.35 µs | 55.88 µs | −10.4% |
| `08-words/all-english` | 2.76 ms | 2.54 ms | −8.0% |
| `14-quadratic/2x` | 43.96 µs | 40.36 µs | −8.2% |
| `14-quadratic/1x` | 15.27 µs | 14.10 µs | −7.7% |
| `10-bounded-repeat/letters-en` | 415.07 µs | 387.04 µs | −6.8% |
| `04-ruff-noqa/real` | 230.78 ms | 215.73 ms | −6.5% |

Compile points are unchanged (±0.7%), as they should be. rurex's overall
score against the fastest engine improves from ×2.65 to ×2.59; the
`08-words` group goes from ×1.62 to ×1.54, `14-quadratic` from ×1.85 to
×1.63, and `12-dictionary` reaches ×1.00 (fastest engine).

The project suite (`make bench`, `docs/benchmarks.md`) was **not**
republished: two runs of identical code differed by 16–32% on the
scan-bound points and by ~20% on the thread points, so publishing either
would have shown noise as a regression. The previous snapshot stands,
and the A/B probe above is the evidence for this change. See the
follow-up note below.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0017-thread-local-capture-locations.md`:
  per-thread scratch keyed by a never-reused id, four slots with
  round-robin eviction, and why a `Regex` field (lock or `RefCell`), a
  single slot, a global map, or pointer keying are all worse.
- [acted] `spec/docs/architecture.md` "Matching path" records the
  scratch and where the id comes from.
- [acted] `spec/docs/index.md` catalogues ADR-0017.
- [acted] `docs/rebar-benchmark.md`/`.html` and its four charts were
  regenerated from the new run.
- [acted] The `endpos = 0` fix carries its own commit and a test; no
  document described the buggy behaviour.

### Future-task notes

- [open] More than four patterns in rotation on one thread evict each
  other's slot, and each call then rebuilds `capture_locations()` — the
  old per-call cost, not worse, but the win disappears. Raise the slot
  count only with a measurement that shows such a workload.
- [open] `src/retry.rs` still allocates its slot vector per zero-width
  retry (`vec![None; (groups + 1) * 2]`) and creates a PikeVM cache per
  call. Empty-match-heavy iteration (already a plan-level concern in
  `report-1792240544-zero-width-fix.md`) has the same shape of fix
  available.
- [open] The project suite's scan-bound points swing 16–32% between runs
  of identical code on `regexus`, which makes it unusable for changes in
  the 5–10% range. Min-of-N inside a point is not enough; it needs
  repeated full runs merged by median (the mechanism
  `merge_runs.py` provides for the Telegram suite) or a quieter host.
- [open] `perf` is not installed on `regexus` (`perf_event_paranoid=3`),
  so instruction counts are only available locally. Installing it would
  let attribution (rather than A/B guessing) happen on the benchmark
  host.

### Tooling/process

- Alternating wheels in one lock session (`pip install
  --force-reinstall --no-deps` per wheel, one measurement per wheel per
  round) is the cheapest reliable A/B for per-call changes: it removes
  session-to-session drift from the comparison. The suite's run-to-run
  noise here was far larger than the effect being measured.
- `benchmarks/run.py --test <name>` **overwrites** `benchmarks/results.json`
  with just that test. Running it for a targeted re-measure destroys the
  published snapshot; a full `make bench` is needed to restore it. Worth
  a guard (or an `--out` flag) before the next targeted run.
- `git apply --cached --recount` with a hand-extracted hunk is a workable
  way to split a reviewable bug fix out of a working tree that also
  carries a larger performance change, without stashing or rebuilding.
