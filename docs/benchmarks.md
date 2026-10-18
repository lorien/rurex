# Benchmarks: `rurex` vs `re`

Nine single-variable benchmark tests comparing the linear-time engine
to the standard library's backtracking engine, measured end to end from
Python — including every bit of object construction a real user pays
for. Every test varies exactly one parameter (stated in its title) and
every chart has two axes: the **variable parameter** on X and **time
per operation** on Y.

- **Test 1 — Scan vs haystack size:** rurex **×31–35 faster** up to
  4 MB and **×15** at 16 MB, and both engines scale linearly.
- **Test 2 — Scan vs needle position:** with the needle in the very
  first bytes both engines answer in well under a microsecond (`re`
  163 ns, rurex 294 ns); from a quarter of the text onward rurex is
  ×17–34 faster.
- **Test 3 — Extraction vs haystack size:** rurex ahead ×2.3–2.5 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** rurex ahead at every
  measured density — ×7.4 when matches are sparse, ×1.7 at the densest.
- **Test 5 — Pathological:** `re` never finishes; rurex answers in
  **~200 ns** at any size.
- **Test 6 — Threads on GIL Python:** a fixed pile of 8000 scans
  split across 1–8 workers — `re` starts at 10.0 s and only worsens
  (19.0 s at 8 workers; the GIL serializes it), while rurex drops from
  0.32 s to 0.08 s because its scans run without the GIL.
- **Test 7 — Threads on free-threaded Python:** the same pile on
  3.14t (GIL off) — both engines scale ~4× and rurex keeps a
  11–14× lead at every point.
- **Test 8 — Needle position × 4 threads on GIL Python:** Test 2's
  8 MB haystack as a 1000-search pile across 4 workers — `re` pays
  its serialized per-op cost at every position (×69–97 slower);
  rurex's scans do use the workers, gaining 2.2–3.9× over its
  serialized work.
- **Test 9 — Needle position × 4 threads on free-threaded Python:**
  the same pile on 3.14t — both engines scale with the workers and
  rurex stays ×9.2–13.5 ahead at every position.

All benchmark runners hold `/tmp/core4.lock` for the whole matrix, so
two runs cannot share the designated physical cores. The lease is
host-local, mode 0666, and records user, PID, host, CPU list, start time
and command; a waiter prints the owner and fails after the timeout.
Each point also tracks CPU efficiency and retries contaminated points
before failing.

A standalone HTML twin of this page (images embedded) is written
alongside it: `python benchmarks/make_charts.py` produces
`docs/benchmarks.html`. It embeds the charts as base64, so it is a
self-contained snapshot and is only as fresh as its last run — always
regenerate it together with the charts (`--no-html` opts out).

Two real-world complements, each one cold run per measured point:
@mention extraction over 1M Telegram posts (short texts, extraction
bound) in [`telegram-benchmark.md`](telegram-benchmark.md), and
frontend HTML `href`/`src` extraction (long documents, dense grouped
extraction) in [`homepage-benchmark.md`](homepage-benchmark.md).

## Methodology

- **One variable per test.** Each test fixes its context (haystack
  size, density, ...) as stated in the title and sweeps a single
  parameter; charts plot parameter vs time.
- **End-to-end timing.** The Python-visible operation is timed
  (method call → returned objects), not the engine in isolation.
  rurex's per-match Python-object construction is included — that is
  the honest number a user experiences.
- **Timing protocol.** `timeit`-style auto-calibrated iteration counts
  (≥ 0.2 s of work per measurement), median of 3 repeats. The
  pathological test is single-shot under a 2 s timeout guard on `re`,
  because its runtime explodes.
- **Release build.** All rurex numbers use `maturin develop
  --release` (opt-level 3, fat LTO).
- **Build check.** `make bench` rebuilds the release extension before
  measuring, because a debug extension left installed silently makes
  every rurex number roughly 2.5× slower while leaving `re`'s numbers
  untouched. `results.json` records a constant `build` string, so it
  cannot prove which binary was measured; distrust a run whose rurex
  numbers moved but `re`'s did not. The previous edition of this page
  predates that rebuild step.
- **Deterministic inputs.** Haystacks are generated from fixed seeds;
  needle positions and densities are stated per test.
- **Threading protocol (Tests 6–9).** A fixed pile of searches is
  split evenly across barrier-started worker threads; the chart
  plots wall-clock time for the whole pile, so a dropping line means
  threads turned into throughput. One measurement per point (each is
  an aggregate of thousands of ops); Tests 7 and 9 run in a
  free-threaded 3.14t subprocess orchestrated by the harness. Tests
  6–7 sweep the worker count (pile of 8000 over a 1 MB haystack);
  Tests 8–9 fix 4 workers and sweep the needle's position (pile of
  1000 over Test 2's 8 MB haystack). Position 0% is excluded from the
  threaded sweeps: a ~160 ns `re` op gives the whole pile ~0.2 ms of
  work — thread startup alone costs milliseconds, so that point
  would time the harness, not the engines. The 1 MB haystack is
  cache-resident on this CPU, which flatters absolute scan throughput
  equally for both engines; the 8 MB haystack exceeds the CPU's
  6 MB L3, so four parallel scanners share memory bandwidth (one
  reason Tests 8–9 scale below the ideal 4×).

## Environment

| | |
|---|---|
| CPU | Intel Core i5-8350U @ 1.70 GHz (4 cores / 8 threads) |
| OS | Linux 6.12 (Debian 13) |
| Python | CPython 3.13.5 |
| rurex | 0.1.0, release profile (opt-level=3, LTO=fat) |
| opponent | stdlib `re` (same interpreter; Tests 6–7 also on
  free-threaded CPython 3.14.8) |

Numbers are machine-specific; expect different absolute values and
somewhat different ratios elsewhere.

---

## Test 1 — Scan: haystack size (needle at the end)

A plain literal search for a single needle occurring once, with the
needle at the very end of the text — the worst case, the whole
haystack must be scanned. Variable: haystack size from 64 KB to
16 MB. This is the classic log-grep workload: rurex scans with
SIMD-accelerated prefilters, `re` steps its backtracking VM character
by character.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — first match anywhere; timed per call.

**Data example** (word filler, one needle at the end)

```
jumps delta data node item brown lorem fox alpha item world alpha text
amet node lazy fox alpha the the-needle-string …
```

![Scan vs size](img/bench_scan_size_data_size.png)

| Size | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 64 KB | 76.6 µs | 2.3 µs | rurex ×33.2 |
| 256 KB | 312.7 µs | 8.9 µs | rurex ×35.3 |
| 1024 KB | 1.24 ms | 36.3 µs | rurex ×34.1 |
| 4096 KB | 4.99 ms | 159.0 µs | rurex ×31.4 |
| 16384 KB | 20.17 ms | 1.32 ms | rurex ×15.3 |

Both engines scale linearly; rurex' constant is more than an order of
magnitude smaller. The advantage halves at 16 MB as scanning becomes
memory-bound — the prefilter's CPU advantage stops mattering when both
engines wait on RAM.

---

## Test 2 — Scan: needle position (fixed 8 MB haystack)

The same single-needle literal search as Test 1, but the haystack is
fixed at 8 MB and the variable is where in the text the needle sits,
from the first bytes (0%) to the very end (100%). Position 0% favors
`re`'s instant fast path; every later position is scan-bound.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — first match anywhere; timed per call.

**Data example** (needle mid-text, as at position ~50%)

```
… dog lorem beta jumps the-needle-string gamma alpha data sit lorem …
```

![Scan vs position](img/bench_scan_position_match_position.png)

| Needle at | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 0% | 163 ns | 294 ns | `re` ×1.8 |
| 25% | 2.49 ms | 73.1 µs | rurex ×34.1 |
| 50% | 4.99 ms | 169.9 µs | rurex ×29.4 |
| 75% | 7.53 ms | 341.3 µs | rurex ×22.1 |
| 100% | 10.11 ms | 600.7 µs | rurex ×16.8 |

The closest point of the race is a needle in the very first bytes:
`re`'s fast path answers in 163 ns and rurex in 294 ns — both well
under a microsecond, both invisible to any real workload. Past the
first quarter of the text, rurex' scan speed dominates by ×17–34.
(An earlier build showed a ~0.7 ms floor here — every `search` call
was counting the characters of the whole haystack to fill
`Match.endpos` even when the match sat in the first bytes; `endpos` is
now computed lazily, on first read.)

---

## Test 3 — Extraction: haystack size (fixed density, 80 matches/KB)

Digit-run extraction (`findall`) over word filler at a fixed density
of 80 matches per KB. Variable: haystack size from 64 KB to 4 MB (the
match count grows with it). This is the match-dense regime, where
building the returned values — not scanning — dominates.

**Pattern**

```python
r"\d{3,4}"
```

**Method:** `.findall(text)` — one call returns every match, as a list of strings.

**Data example** (digits in word filler at 80/KB)

```
7685 value word word beta beta lorem node alpha word value text value
alpha word value value node al …
```

![Extraction vs size](img/bench_extract_size_data_size.png)

| Size | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 64 KB | 1.37 ms | 599.9 µs | rurex ×2.3 |
| 256 KB | 5.89 ms | 2.34 ms | rurex ×2.5 |
| 1024 KB | 25.15 ms | 10.86 ms | rurex ×2.3 |
| 4096 KB | 103.39 ms | 45.61 ms | rurex ×2.3 |

At this density rurex is steadily ×2.3–2.5 ahead: `findall` builds only
the returned values (strings, or one string per group) in one pass —
no intermediate match objects.

---

## Test 4 — Extraction: match density (fixed 1 MB haystack)

The same digit-run extraction as Test 3, but the haystack is fixed at
1 MB and the variable is match density, from 5 to 160 matches per KB.
This isolates the per-match cost from the scan cost: sparse
matches are scan-bound, dense matches are extraction-bound.

**Pattern**

```python
r"\d{3,4}"
```

**Method:** `.findall(text)` — one call returns every match, as a list of strings.

**Data example** (digits in word filler at 160/KB)

```
7685 4432 9766 word alpha 6210 5510 text lorem 5965 8533 word value …
```

![Extraction vs density](img/bench_extract_density_match_density.png)

| Density | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 5 /KB | 16.67 ms | 2.25 ms | rurex ×7.4 |
| 20 /KB | 19.12 ms | 3.71 ms | rurex ×5.2 |
| 80 /KB | 25.34 ms | 10.99 ms | rurex ×2.3 |
| 160 /KB | 33.32 ms | 19.72 ms | rurex ×1.7 |

With sparse matches, scanning dominates and rurex is up to 7.4×
faster; as matches multiply, the per-match cost of building the
returned values takes over and the lead narrows to ×1.7 — rurex still
wins at every measured density, but the margin is extraction-bound.

---

## Test 5 — Pathological: catastrophic backtracking

The pattern `(x+x+)+y` never matches a run of x's — and for a
backtracking engine, the number of ways to distribute the x's across
the nested groups grows exponentially: `re`'s time doubles with every
two extra input characters. A finite-automata engine cannot backtrack,
so rurex' time is flat. This is a **safety property** (ReDoS
resistance), not a speed feature: no pattern and no input can make
rurex blow up.

**Pattern**

```python
r"(x+x+)+y"
```

**Method:** `.search(text)` — one single call per point, no timed loop
(`re` wrapped in a 2 s timeout guard).

**Data example**

```
xxxxxxxxxxxxxxxxxxxxxxxx    (n = 24 of the sweep; no y — nothing matches)
```

![Pathological vs length](img/bench_pathological_input_length.png)

| n | `re` | rurex | Speedup |
|---:|---:|---:|---:|
| 8 | 22.4 µs | 177 ns | rurex ×126 |
| 10 | 66.7 µs | 181 ns | rurex ×368 |
| 12 | 237.5 µs | 188 ns | rurex ×1,260 |
| 14 | 929.4 µs | 188 ns | rurex ×4,931 |
| 16 | 3.68 ms | 193 ns | rurex ×19,078 |
| 18 | 14.49 ms | 196 ns | rurex ×73,760 |
| 20 | 58.00 ms | 198 ns | rurex ×292,633 |
| 22 | 231.04 ms | 202 ns | rurex ×1,145,131 |
| 24 | 921.74 ms | 203 ns | rurex ×4,530,714 |
| 26 | *≥ 2 s — still running at the timeout* | 210 ns | — |
| 28 | *≥ 2 s — still running at the timeout* | 213 ns | — |

Extrapolating `re`'s doubling curve, n=40 would take longer than the
age of the universe. rurex answers every input in about 200 ns, and
would keep doing so at n=40, n=400, n=4 million.

---

## Test 6 — Concurrency: threads on GIL Python

The scan workload of Test 1 as one fixed pile — 8000 whole-haystack
searches over a 1 MB haystack, needle at the end — split evenly
across N barrier-started worker threads. Y is wall-clock time for
the whole pile: a dropping line means threads became throughput.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — 8000 calls split across N threads;
Y is the wall-clock for the whole pile.

![Threads on GIL Python](img/bench_threads_gil_threads.png)

| Workers | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 1 | 9.98 s | 0.32 s | rurex ×30.8 |
| 2 | 10.31 s | 0.15 s | rurex ×68.9 |
| 4 | 15.02 s | 0.11 s | rurex ×135.6 |
| 8 | 19.01 s | 0.08 s | rurex ×230.8 |

`re` never buys throughput from threads on GIL Python: it never beats
its one-worker time (10.0 s) and deteriorates to 19.0 s at eight
workers, because the GIL serializes every search and contention adds
overhead on top. rurex releases the GIL around its scans (1 MB
haystacks sit far above the 4 KB release threshold), so its pile
shrinks 3.9× from one worker to eight — the four physical cores
nearly saturated — even though every call must re-acquire the GIL to
build its `Match`. The lead grows from ×31 at one worker to ×231 at
eight.

---

## Test 7 — Concurrency: threads on free-threaded Python

The same pile as Test 6 — 8000 searches, split across N workers,
wall-clock for the pile — on free-threaded CPython 3.14t with the
GIL off.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — 8000 calls split across N threads;
Y is the wall-clock for the whole pile.

![Threads on free-threaded Python](img/bench_threads_free_threads.png)

| Workers | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 1 | 11.35 s | 0.88 s | rurex ×12.9 |
| 2 | 5.86 s | 0.41 s | rurex ×14.2 |
| 4 | 3.13 s | 0.28 s | rurex ×11.3 |
| 8 | 2.63 s | 0.21 s | rurex ×12.3 |

Without the GIL both engines scale: `re` chews the pile 4.3× faster
at 8 workers than at 1, rurex 4.1× (both saturating this
4-core/8-thread machine). And rurex keeps its single-thread lead at
every point — 0.21 s vs 2.63 s at 8 workers. The single-threaded
columns also show the free-threaded interpreter's cost honestly:
`re` 11.4 s here vs 10.0 s on GIL Python, and rurex 0.88 s vs
0.32 s — per-object locking makes the Python side of a rurex call
2.7× dearer on the free-threaded build, while `re`'s C-level match
objects barely notice.

---

## Test 8 — Concurrency: needle position on GIL Python (4 workers)

Test 2's scan workload as one fixed pile — 1000 whole-haystack
searches over the 8 MB haystack, split evenly across 4 barrier-
started workers — with the needle's position as the variable
(25–100%). Position 0% is excluded: a ~160 ns `re` op gives the whole
pile ~0.2 ms of work, less than thread startup itself, so that point
would time the harness, not the engines.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — 1000 calls split across 4 fixed threads;
Y is the wall-clock for the whole pile.

![Threads × needle position on GIL Python](img/bench_threads_pos_gil_match_position.png)

| Needle at | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 25% | 2.58 s | 32.7 ms | rurex ×78.9 |
| 50% | 5.33 s | 54.9 ms | rurex ×97.1 |
| 75% | 8.00 s | 107.2 ms | rurex ×74.6 |
| 100% | 10.57 s | 154.1 ms | rurex ×68.6 |

`re`'s wall equals its serialized per-op cost — Test 2's column ×
1000 plus a few percent of contention: the GIL serializes every
search, so four workers buy nothing and the pile grows linearly with
the needle's position. rurex stays 69–97× ahead at every position,
and the four workers do help its scans: 2.2× over its serialized work
at 25% (73 ms → 33 ms) and 3.9× at 100% (601 ms → 154 ms), because
the GIL is released for the scan itself. The gain is smallest at the
near positions, where the serialized Python side — building the
`Match`, converting spans — is most of the call; at 100% the scans
dominate and the four cores show up.

---

## Test 9 — Concurrency: needle position on free-threaded Python (4 workers)

The same pile as Test 8 — 1000 searches over the 8 MB haystack, 4
workers, needle position 25–100% — on free-threaded CPython 3.14t
with the GIL off.

**Pattern**

```python
"the-needle-string"
```

**Method:** `.search(text)` — 1000 calls split across 4 fixed threads;
Y is the wall-clock for the whole pile.

![Threads × needle position on free-threaded Python](img/bench_threads_pos_free_match_position.png)

| Needle at | `re` | rurex | Speedup |
|---|---:|---:|---:|
| 25% | 795.8 ms | 58.9 ms | rurex ×13.5 |
| 50% | 1.60 s | 142.0 ms | rurex ×11.3 |
| 75% | 2.42 s | 262.8 ms | rurex ×9.2 |
| 100% | 3.33 s | 341.7 ms | rurex ×9.7 |

Without the GIL both engines turn the four workers into throughput at
every position — 1000 searches over 8 MB finish in 342 ms for rurex
and 3.33 s for `re` at 100% — and rurex keeps a 9.2–13.5× lead. Both
fall short of the ideal 4× in part because the 8 MB haystack exceeds
this CPU's 6 MB L3, so four parallel scanners share memory
bandwidth. The lead is narrower than Test 8's because the
free-threaded interpreter makes rurex's per-call Python side 2.7×
dearer (Test 7: 0.88 s vs 0.32 s for the same pile), and these piles
are short: 1000 operations, not 8000.

---

## Reproducing

```sh
make venv && source .venv/bin/activate   # or: python -m venv .venv + pip install
maturin develop --release          # release build — mandatory

# Test 7 needs a free-threaded interpreter with a release rurex:
uv venv --python 3.14t .venv-t
VIRTUAL_ENV=.venv-t uv pip install maturin pytest
.venv-t/bin/maturin develop --release

python benchmarks/run.py           # runs all 7 tests (spawns .venv-t for Test 7)
python benchmarks/make_charts.py   # charts + docs/benchmarks.html twin
```

`benchmarks/suite.py` defines the nine tests (patterns, data
generators, the single variable and fixed context per test);
`benchmarks/run.py` is the harness (auto-calibration, medians,
timeout guard, environment capture); `benchmarks/make_charts.py`
regenerates the charts and the HTML render from `results.json`. This
page's numbers come from the environment above; regenerate on your own
hardware before making decisions that depend on them.

---

*Raw results: [`benchmarks/results.json`](../benchmarks/results.json).
Numbers on this page were measured on 2026-10-19 with rurex 0.1.0.*
