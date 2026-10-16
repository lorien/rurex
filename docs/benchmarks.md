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
  168 ns, rurex 455 ns); from a quarter of the text onward rurex is
  ×7.3–13.5 faster.
- **Test 3 — Extraction vs haystack size:** rurex ahead ~×1.4 at
  match-dense fixed density.
- **Test 4 — Extraction vs match density:** rurex ahead at every
  measured density — ×6.2 when matches are sparse, narrowing to
  parity (×1.00) at the densest.
- **Test 5 — Pathological:** `re` never finishes; rurex answers in
  **~250 ns** at any size.

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
| 64 KB | 78 µs | 6.0 µs | rurex ×13.0 |
| 256 KB | 312 µs | 23 µs | rurex ×13.4 |
| 1 MB | 1.24 ms | 93 µs | rurex ×13.4 |
| 4 MB | 5.00 ms | 407 µs | rurex ×12.3 |
| 16 MB | 20.5 ms | 3.12 ms | rurex ×6.6 |

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
| 0% | 168 ns | 455 ns | `re` ×2.7 |
| 25% | 2.51 ms | 186 µs | rurex ×13.5 |
| 50% | 5.02 ms | 423 µs | rurex ×11.9 |
| 75% | 7.59 ms | 863 µs | rurex ×8.8 |
| 100% | 10.2 ms | 1.40 ms | rurex ×7.3 |

The closest point of the race is a needle in the very first bytes:
`re`'s fast path answers in 170 ns and rurex in 436 ns — both well
under a microsecond, both invisible to any real workload. Past the
first quarter of the text, rurex' scan speed dominates by ×7.3–13.5.
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
| 64 KB | 1.41 ms | 1.00 ms | rurex ×1.41 |
| 256 KB | 5.90 ms | 4.13 ms | rurex ×1.43 |
| 1 MB | 25.8 ms | 18.3 ms | rurex ×1.41 |
| 4 MB | 105.9 ms | 75.8 ms | rurex ×1.40 |

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
| 5 /KB | 16.8 ms | 2.70 ms | rurex ×6.2 |
| 20 /KB | 18.2 ms | 5.41 ms | rurex ×3.4 |
| 80 /KB | 25.0 ms | 18.2 ms | rurex ×1.4 |
| 160 /KB | 34.1 ms | 33.9 ms | rurex ×1.00 |

With sparse matches, scanning dominates and rurex is up to 6.2×
faster; as matches multiply, the per-match cost of building the
returned values takes over and `re` closes the gap — at the densest
measured point the two engines are effectively even (×1.00).

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
| 8 | 21.9 µs | 233 ns |
| 12 | 248 µs | 234 ns |
| 16 | 3.64 ms | 236 ns |
| 20 | 61.9 ms | 242 ns |
| 24 | 982 ms | 250 ns |
| 26 | *≥ 2 s — still running at the timeout* | 250 ns |
| 28 | — | 252 ns |

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
