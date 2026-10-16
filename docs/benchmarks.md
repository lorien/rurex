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
  179 ns, rurex 471 ns); from a quarter of the text onward rurex is
  ×7.4–12.7 faster.
- **Test 3 — Extraction vs haystack size:** rurex ahead ~×1.4 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** rurex ahead at every
  measured density — ×6.3 when matches are sparse, ×1.2–1.3 at the densest.
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
| 64 KB | 82 µs | 6.3 µs | rurex ×13.0 |
| 256 KB | 329 µs | 25 µs | rurex ×13.0 |
| 1 MB | 1.32 ms | 101 µs | rurex ×13.0 |
| 4 MB | 5.34 ms | 498 µs | rurex ×10.7 |
| 16 MB | 21.9 ms | 3.37 ms | rurex ×6.5 |

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
| 0% | 179 ns | 471 ns | `re` ×2.6 |
| 25% | 2.65 ms | 208 µs | rurex ×12.7 |
| 50% | 5.53 ms | 538 µs | rurex ×10.3 |
| 75% | 8.10 ms | 1.01 ms | rurex ×8.0 |
| 100% | 11.1 ms | 1.49 ms | rurex ×7.4 |

The closest point of the race is a needle in the very first bytes:
`re`'s fast path answers in 170 ns and rurex in 436 ns — both well
under a microsecond, both invisible to any real workload. Past the
first quarter of the text, rurex' scan speed dominates by ×7.4–12.7.
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
| 64 KB | 1.66 ms | 1.14 ms | rurex ×1.45 |
| 256 KB | 6.59 ms | 4.59 ms | rurex ×1.44 |
| 1 MB | 30.1 ms | 21.5 ms | rurex ×1.40 |
| 4 MB | 119.4 ms | 88.1 ms | rurex ×1.36 |

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
| 5 /KB | 19.8 ms | 3.12 ms | rurex ×6.3 |
| 20 /KB | 24.1 ms | 6.21 ms | rurex ×3.9 |
| 80 /KB | 30.3 ms | 24.3 ms | rurex ×1.2 |
| 160 /KB | 48.0 ms | 38.0 ms | rurex ×1.3 |

With sparse matches, scanning dominates and rurex is up to 6.3×
faster; as matches multiply, the per-match cost of building the
returned values takes over and `re` closes the gap — at the densest
measured points the two engines are within ~×1.3 of each other
(run-to-run noise on this machine moves this band by ±0.3).

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
| 8 | 26.4 µs | 261 ns |
| 12 | 243 µs | 261 ns |
| 16 | 3.89 ms | 265 ns |
| 20 | 63.4 ms | 267 ns |
| 24 | 1.04 s | 269 ns |
| 26 | *≥ 2 s — still running at the timeout* | 269 ns |
| 28 | — | 313 ns |

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
