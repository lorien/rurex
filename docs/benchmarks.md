# Benchmarks: `rurex` vs `re`

Five single-variable benchmark tests comparing the linear-time engine
to the standard library's backtracking engine, measured end to end from
Python — including every bit of object construction a real user pays
for. Every test varies exactly one parameter (stated in its title) and
every chart has two axes: the **variable parameter** on X and **time
per operation** on Y.

- **Test 1 — Scan vs haystack size:** rurex up to **8.8× faster**, and
  both engines scale linearly.
- **Test 2 — Scan vs needle position:** the one regime where `re` wins
  (needle in the first bytes); rurex wins ×2.8–4.7 from a quarter of
  the text onward.
- **Test 3 — Extraction vs haystack size:** `re` ahead by ~×1.3 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** the honest crossover —
  rurex ×4.3 faster when matches are sparse, `re` ×2.0 faster when
  they are dense.
- **Test 5 — Pathological:** `re` never finishes; rurex answers in
  **~240 ns** at any size.

A rendered, standalone HTML twin of this page (images embedded) lives
at [`benchmarks.html`](benchmarks.html) — open it directly in a
browser.

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
| 64 KB | 77 µs | 9.0 µs | rurex ×8.5 |
| 256 KB | 315 µs | 36 µs | rurex ×8.8 |
| 1 MB | 1.25 ms | 146 µs | rurex ×8.6 |
| 4 MB | 5.05 ms | 693 µs | rurex ×7.3 |
| 16 MB | 20.7 ms | 5.07 ms | rurex ×4.1 |

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
| 0% | 166 ns | 744 µs | `re` ×4480 |
| 25% | 2.5 ms | 902 µs | rurex ×2.8 |
| 50% | 5.0 ms | 1.23 ms | rurex ×4.1 |
| 75% | 7.6 ms | 1.61 ms | rurex ×4.7 |
| 100% | 10.2 ms | 2.18 ms | rurex ×4.7 |

The one place `re` wins is a needle in the very first bytes: its fast
path returns almost instantly, while rurex pays a fixed cost for
setting up the capture machinery on the haystack (~0.7 ms here —
visible as the floor of rurex' line). Past the first quarter of the
text, rurex' scan speed dominates.

---

## Test 3 — Extraction: haystack size (fixed density, 80 matches/KB)

Digit-run extraction (`findall`) over word filler at a fixed density
of 80 matches per KB. Variable: haystack size from 64 KB to 4 MB (the
match count grows with it). This is the match-dense regime, where
building Python match objects — not scanning — dominates.

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
| 64 KB | 1.5 ms | 2.0 ms | `re` ×1.3 |
| 256 KB | 6.0 ms | 8.0 ms | `re` ×1.3 |
| 1 MB | 24.6 ms | 32.1 ms | `re` ×1.3 |
| 4 MB | 101.3 ms | 128.9 ms | `re` ×1.3 |

At this density `re` is steadily ~1.3× ahead: per-match object
construction dominates, and its C loop is cheaper than rurex'
Python-level result assembly.

---

## Test 4 — Extraction: match density (fixed 1 MB haystack)

The same digit-run extraction as Test 3, but the haystack is fixed at
1 MB and the variable is match density, from 5 to 160 matches per KB.
This isolates the per-match cost from the scan cost and shows the
crossover: sparse matches are scan-bound (rurex territory), dense
matches are extraction-bound (`re` territory).

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
| 5 /KB | 16.7 ms | 3.9 ms | rurex ×4.3 |
| 20 /KB | 18.4 ms | 9.9 ms | rurex ×1.9 |
| 80 /KB | 25.1 ms | 31.9 ms | `re` ×1.3 |
| 160 /KB | 30.7 ms | 60.4 ms | `re` ×2.0 |

This is the honest crossover chart. With sparse matches, scanning
dominates and rurex is up to 4.3× faster; as matches multiply, the
per-match cost takes over and `re` wins by up to 2.0×. Your
workload's density decides which engine to use.

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
| 8 | 22 µs | 217 ns |
| 12 | 240 µs | 221 ns |
| 16 | 3.7 ms | 225 ns |
| 20 | 58.6 ms | 229 ns |
| 24 | 928 ms | 236 ns |
| 26 | *≥ 2 s — still running at the timeout* | 241 ns |
| 28 | — | 246 ns |

Extrapolating `re`'s doubling curve, n=40 would take longer than the
age of the universe. rurex answers every input in a quarter of a
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
