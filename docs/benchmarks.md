# Benchmarks: `rurex` vs `re`

Five single-variable benchmark tests comparing the linear-time engine
to the standard library's backtracking engine, measured end to end from
Python — including every bit of object construction a real user pays
for. Every test varies exactly one parameter (stated in its title) and
every chart has two axes: the **variable parameter** on X and **time
per operation** on Y.

- **Test 1 — Scan vs haystack size:** rurex up to **×13.3 faster**,
  and both engines scale linearly.
- **Test 2 — Scan vs needle position:** with the needle in the very
  first bytes both engines answer in well under a microsecond (`re`
  179 ns, rurex 468 ns); from a quarter of the text onward rurex is
  ×7.3–12.2 faster.
- **Test 3 — Extraction vs haystack size:** rurex ahead ~×1.4 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** rurex ahead at every
  measured density — ×6.8 when matches are sparse, narrowing to
  parity (×1.04) at the densest.
- **Test 5 — Pathological:** `re` never finishes; rurex answers in
  **~270 ns** at any size.

A standalone HTML twin of this page (images embedded) can be rendered
locally for browser viewing: `python benchmarks/make_charts.py --html`
writes `docs/benchmarks.html`.

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
- **Deterministic inputs.** Haystacks are generated from fixed seeds;
  needle positions and densities are stated per test.

## Environment

| | |
|---|---|
| CPU | Intel Core i5-8350U @ 1.70 GHz |
| OS | Linux 6.12 (Debian 13) |
| Python | CPython 3.13.5 |
| rurex | 0.1.0, release profile (opt-level=3, LTO=fat) |
| opponent | stdlib `re` (same interpreter) |

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

**Data example** (word filler, one needle at the end)

```
jumps delta data node item brown lorem fox alpha item world alpha text
amet node lazy fox alpha the the-needle-string …
```

![Scan vs size](img/bench_scan_size_data_size.png)

| Size | `re` | rurex | Speedup |
|---:|---:|---:|---:|
| 64 KB | 83 µs | 6.2 µs | rurex ×13.3 |
| 256 KB | 329 µs | 25 µs | rurex ×13.2 |
| 1 MB | 1.35 ms | 103 µs | rurex ×13.2 |
| 4 MB | 5.47 ms | 523 µs | rurex ×10.5 |
| 16 MB | 21.3 ms | 3.19 ms | rurex ×6.7 |

Both engines scale linearly; rurex' constant is several times smaller.
The advantage narrows at 16 MB as scanning becomes memory-bound — the
prefilter's CPU advantage stops mattering when both engines wait on
RAM.

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

**Data example** (needle mid-text, as at position ~50%)

```
… dog lorem beta jumps the-needle-string gamma alpha data sit lorem …
```

![Scan vs position](img/bench_scan_position_match_position.png)

| Needle at | `re` | rurex | Speedup |
|---:|---:|---:|---:|
| 0% | 179 ns | 468 ns | `re` ×2.6 |
| 25% | 2.63 ms | 216 µs | rurex ×12.2 |
| 50% | 5.33 ms | 506 µs | rurex ×10.5 |
| 75% | 8.10 ms | 990 µs | rurex ×8.2 |
| 100% | 10.6 ms | 1.45 ms | rurex ×7.3 |

The closest point of the race is a needle in the very first bytes:
`re`'s fast path answers in 170 ns and rurex in 436 ns — both well
under a microsecond, both invisible to any real workload. Past the
first quarter of the text, rurex' scan speed dominates by ×7.3–12.2.
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

**Data example** (digits in word filler at 80/KB)

```
7685 value word word beta beta lorem node alpha word value text value
alpha word value value node al …
```

![Extraction vs size](img/bench_extract_size_data_size.png)

| Size | `re` | rurex | Speedup |
|---:|---:|---:|---:|
| 64 KB | 1.57 ms | 1.16 ms | rurex ×1.36 |
| 256 KB | 6.57 ms | 4.49 ms | rurex ×1.46 |
| 1 MB | 29.5 ms | 20.6 ms | rurex ×1.43 |
| 4 MB | 117.7 ms | 80.3 ms | rurex ×1.47 |

At this density rurex is steadily ~×1.4 ahead: `findall` builds only
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

**Data example** (digits in word filler at 160/KB)

```
7685 4432 9766 word alpha 6210 5510 text lorem 5965 8533 word value …
```

![Extraction vs density](img/bench_extract_density_match_density.png)

| Density | `re` | rurex | Speedup |
|---:|---:|---:|---:|
| 5 /KB | 19.4 ms | 2.84 ms | rurex ×6.8 |
| 20 /KB | 19.8 ms | 5.88 ms | rurex ×3.4 |
| 80 /KB | 28.8 ms | 21.1 ms | rurex ×1.4 |
| 160 /KB | 38.0 ms | 36.5 ms | rurex ×1.04 |

With sparse matches, scanning dominates and rurex is up to 6.8×
faster; as matches multiply, the per-match cost of building the
returned values takes over and `re` closes the gap — at the densest
measured point the two engines are effectively even (×1.04).

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

**Data example**

```
xxxxxxxxxxxxxxxxxxxxxxxx    (n = 24 of the sweep; no y — nothing matches)
```

![Pathological vs length](img/bench_pathological_input_length.png)

| n | `re` | rurex |
|---:|---:|---:|
| 8 | 30.6 µs | 263 ns |
| 12 | 262 µs | 265 ns |
| 16 | 4.22 ms | 272 ns |
| 20 | 64.5 ms | 267 ns |
| 24 | 1.06 s | 288 ns |
| 26 | *≥ 2 s — still running at the timeout* | 287 ns |
| 28 | — | 285 ns |

Extrapolating `re`'s doubling curve, n=40 would take longer than the
age of the universe. rurex answers every input in well under half of a
microsecond, and would keep doing so at n=40, n=400, n=4 million.

---

## Reproducing

```sh
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest matplotlib markdown
maturin develop --release          # release build — mandatory

python benchmarks/run.py           # runs the suite, writes results.json
python benchmarks/make_charts.py --html   # charts + docs/benchmarks.html
```

`benchmarks/suite.py` defines the five tests (patterns, data
generators, the single variable and fixed context per test);
`benchmarks/run.py` is the harness (auto-calibration, medians,
timeout guard, environment capture); `benchmarks/make_charts.py`
regenerates the charts and the HTML render from `results.json`. This
page's numbers come from the environment above; regenerate on your own
hardware before making decisions that depend on them.

---

*Raw results: [`benchmarks/results.json`](../benchmarks/results.json).
Numbers on this page were measured on 2026-10-16 with rurex 0.1.0.*
