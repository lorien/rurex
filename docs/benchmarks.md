# Benchmarks: `rurex` vs `re`

Nine single-variable benchmark tests comparing the linear-time engine
to the standard library's backtracking engine, measured end to end from
Python — including every bit of object construction a real user pays
for. Every test varies exactly one parameter (stated in its title) and
every chart has two axes: the **variable parameter** on X and **time
per operation** on Y.

- **Test 1 — Scan vs haystack size:** rurex **×30–43 faster**, and
  both engines scale linearly.
- **Test 2 — Scan vs needle position:** with the needle in the very
  first bytes both engines answer in well under a microsecond (`re`
  67 ns, rurex 125 ns); from a quarter of the text onward rurex is
  ×31–33 faster.
- **Test 3 — Extraction vs haystack size:** rurex ahead ×1.8–2.0 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** rurex ahead at every
  measured density — ×4.5 when matches are sparse, ×1.5 at the densest.
- **Test 5 — Pathological:** `re` never finishes; rurex answers in
  **under 100 ns** at any size.
- **Test 6 — Threads on GIL Python:** a fixed pile of 8000 scans
  split across 1–8 workers — `re` sits at 7.9–8.1 s at every worker
  count (the GIL serializes it), while rurex drops from 0.20 s to
  0.045 s because its scans run without the GIL.
- **Test 7 — Threads on free-threaded Python:** the same pile on
  3.14t (GIL off) — both engines scale ~4× and rurex keeps a
  28–43× lead at every point.
- **Test 8 — Needle position × 4 threads on GIL Python:** Test 2's
  8 MB haystack as a 1000-search pile across 4 workers — `re` pays
  its serialized per-op cost at every position (×112–128 slower);
  rurex's scans do use the workers, gaining 3.4–3.9× over its
  serialized work.
- **Test 9 — Needle position × 4 threads on free-threaded Python:**
  the same pile on 3.14t — both engines scale ~3.7× with the workers
  and rurex stays ×28–34 ahead at every position.

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
- **Build check.** `make bench` rebuilds both the GIL and the
  free-threaded extension before measuring, because a debug or stale
  extension silently skews the rurex side and leaves `re` untouched.
  `results.json` records the CPU and, per interpreter, the extension's
  path, size, mtime and sha256; a run whose extension is older than the
  Rust sources prints a warning. Verify a published row by comparing
  that hash with the installed `.so`.
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
  would time the harness, not the engines. Both haystacks (1 MB and
  8 MB) fit this CPU's 24 MB L3, which flatters absolute scan
  throughput equally for both engines; the four pinned workers of
  Tests 8–9 scale to 3.4–3.9× of the ideal 4×, short of it because
  each call also touches memory outside the document.

## Environment

| | |
|---|---|
| CPU | 13th Gen Intel Core i5-13500 (10 cores / 20 threads) |
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
| 64 KB | 54.9 µs | 1.6 µs | rurex ×33.9 |
| 256 KB | 240.1 µs | 5.6 µs | rurex ×42.6 |
| 1024 KB | 980.5 µs | 23.7 µs | rurex ×41.4 |
| 4096 KB | 4.00 ms | 123.0 µs | rurex ×32.5 |
| 16384 KB | 15.87 ms | 521.9 µs | rurex ×30.4 |

Both engines scale linearly; rurex' constant is more than an order of
magnitude smaller. The advantage eases at 16 MB as scanning becomes
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
| 0% | 67 ns | 125 ns | `re` ×1.9 |
| 25% | 1.97 ms | 62.2 µs | rurex ×31.6 |
| 50% | 3.96 ms | 122.2 µs | rurex ×32.4 |
| 75% | 5.99 ms | 183.5 µs | rurex ×32.6 |
| 100% | 7.95 ms | 244.9 µs | rurex ×32.4 |

The closest point of the race is a needle in the very first bytes:
`re`'s fast path answers in 67 ns and rurex in 125 ns — both well
under a microsecond, both invisible to any real workload. Past the
first quarter of the text, rurex' scan speed dominates by ×31–33.
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
| 64 KB | 622.2 µs | 314.9 µs | rurex ×2.0 |
| 256 KB | 2.55 ms | 1.29 ms | rurex ×2.0 |
| 1024 KB | 10.67 ms | 5.79 ms | rurex ×1.8 |
| 4096 KB | 45.01 ms | 24.42 ms | rurex ×1.8 |

At this density rurex is steadily ×1.8–2.0 ahead: `findall` builds only
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
| 5 /KB | 7.18 ms | 1.59 ms | rurex ×4.5 |
| 20 /KB | 7.87 ms | 2.39 ms | rurex ×3.3 |
| 80 /KB | 11.09 ms | 5.72 ms | rurex ×1.9 |
| 160 /KB | 14.31 ms | 9.43 ms | rurex ×1.5 |

With sparse matches, scanning dominates and rurex is up to 4.5×
faster; as matches multiply, the per-match cost of building the
returned values takes over and the lead narrows to ×1.5 — rurex still
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
| 8 | 13.6 µs | 70 ns | rurex ×195 |
| 10 | 39.0 µs | 72 ns | rurex ×544 |
| 12 | 133.2 µs | 75 ns | rurex ×1,782 |
| 14 | 486.7 µs | 76 ns | rurex ×6,374 |
| 16 | 1.93 ms | 83 ns | rurex ×23,198 |
| 18 | 7.68 ms | 88 ns | rurex ×87,661 |
| 20 | 32.78 ms | 87 ns | rurex ×374,687 |
| 22 | 123.70 ms | 91 ns | rurex ×1,362,879 |
| 24 | 496.83 ms | 93 ns | rurex ×5,361,531 |
| 26 | 1.98 s | 95 ns | rurex ×20,781,319 |
| 28 | *≥ 2 s — still running at the timeout* | 98 ns | — |

Extrapolating `re`'s doubling curve, n=40 would take longer than the
age of the universe. rurex answers every input in under 100 ns, and
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
| 1 | 7.87 s | 0.20 s | rurex ×39.9 |
| 2 | 8.08 s | 0.10 s | rurex ×79.9 |
| 4 | 8.08 s | 0.06 s | rurex ×132.3 |
| 8 | 8.09 s | 0.05 s | rurex ×179.2 |

`re` never buys throughput from threads on GIL Python: its pile is
flat at 7.9–8.1 s at every worker count, because the GIL serializes
every search and contention only adds overhead. rurex releases the GIL
around its scans (1 MB haystacks sit far above the 4 KB release
threshold), so its pile shrinks 4.4× from one worker to eight — the
four physical cores it is pinned to, fully used — even though every
call must re-acquire the GIL to build its `Match`. The lead grows from
×40 at one worker to ×179 at eight.

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
| 1 | 8.06 s | 0.19 s | rurex ×42.7 |
| 2 | 4.13 s | 0.10 s | rurex ×41.3 |
| 4 | 2.12 s | 0.06 s | rurex ×34.4 |
| 8 | 1.11 s | 0.04 s | rurex ×27.7 |

Without the GIL both engines scale: `re` chews the pile 7.3× faster
at 8 workers than at 1, rurex 4.7× (the four pinned cores are the
limit). And rurex keeps its single-thread lead at every point —
0.04 s vs 1.11 s at 8 workers. The single-worker columns also show
what the free-threaded build costs: essentially nothing for rurex
(0.19 s here vs 0.20 s on GIL Python) because both extensions are
rebuilt from the same source before a run, and `re` moves only
8.1 s vs 7.9 s.

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
| 25% | 2.02 s | 17.5 ms | rurex ×115.7 |
| 50% | 4.04 s | 36.1 ms | rurex ×112.0 |
| 75% | 6.06 s | 51.4 ms | rurex ×118.0 |
| 100% | 8.07 s | 63.0 ms | rurex ×128.0 |

`re`'s wall equals its serialized per-op cost — Test 2's column ×
1000 plus a few percent of contention: the GIL serializes every
search, so four workers buy nothing and the pile grows linearly with
the needle's position. rurex stays 112–128× ahead at every position,
and the four workers do help its scans: 3.4–3.9× over its serialized
work (62 ms → 17 ms at 25%, 245 ms → 63 ms at 100%), because the GIL
is released for the scan itself. The gain is smallest at the near
positions, where the serialized Python side — building the `Match`,
converting spans — is most of the call; at 100% the scans dominate
and the four cores show up.

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
| 25% | 544.6 ms | 19.6 ms | rurex ×27.7 |
| 50% | 1.10 s | 37.2 ms | rurex ×29.5 |
| 75% | 1.62 s | 47.3 ms | rurex ×34.3 |
| 100% | 2.17 s | 63.9 ms | rurex ×34.0 |

Without the GIL both engines turn the four workers into throughput at
every position — 1000 searches over 8 MB finish in 64 ms for rurex
and 2.17 s for `re` at 100% — and rurex keeps a 27–34× lead. Both
scale about 3.7× against their serialized cost, short of the ideal 4×
because each call also touches memory outside the document. The lead
is narrower than Test 8's because the GIL is what `re` cannot
parallelize there; once the GIL is gone, four workers help `re` as
much as they help rurex.

---

## Reproducing

Run this on the benchmark host, not on a development laptop (see
"Benchmarks" in [`spec/docs/dev.md`](../spec/docs/dev.md)): the machine
is part of the measurement and `results.json` records it.

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
Numbers on this page were measured on 2026-10-19 on the benchmark
host (`regexus`, i5-13500) with rurex 0.1.0; `results.json` records
each interpreter's extension sha256.*
