# rebar: rurex against stdlib `re` and the Rust engine

[rebar](https://github.com/BurntSushi/rebar) is BurntSushi's barometer for
comparing regex engines on a curated set of tasks. This page reports how
rurex fares on it against the two engines a Python user actually weighs
it with: the standard library's `re`, which it replaces, and `rust/regex`,
which it embeds.

The metric is rebar's own: each benchmark divides an engine's median time
by the fastest median among the engines that ran it, and an engine's score
is the geometric mean of those ratios across the benchmarks all three
share. **Lower is faster; 1.0 is the fastest engine on that set.** A score
of ×3 means "typically three times the best engine's time".

## Method

| | |
|---|---|
| Date | 2026-10-21 |
| Host | `regexus`, 13th Gen Intel(R) Core(TM) i5-13500 |
| Isolation | `/tmp/core4.lock` lease (cpus [0, 2, 4, 6]), pinned to CPU 0, governor `performance` |
| rebar | 0.1.0 (curated set, `--max-time 3s --max-iters 10`) |
| rurex | 0.2.0, local wheel built from commit a90c3a5 (per-thread engines, Pattern.count), sha256 a47104da |
| Opponents | stdlib `re`, `rust/regex` (the engine rurex embeds) |
| Checks | every point verified with `rebar measure --test` before timing; no failed points in the data |

| Raw results | sha256 |
|---|---|
| `curated.csv` | 68af6751308b007b… |
| `dict.csv` | 9437e4bed75e1bb6… |

## The ranking

![Geometric mean ranking](img/rebar_rank.png)

| engine | score | benchmarks |
|---|---:|---:|
| `rust/regex` | ×1.08 | 44 |
| `python/rurex` | ×2.58 | 44 |
| `python/re` | ×9.81 | 44 |

rurex sits between the engine it embeds and the module it replaces: about
**3.8× faster than `re`** on average, and
about **2.6× off the Rust engine**. The distance to the engine is
not the scanner: the two run the same automata. It is the Python
boundary — argument handling and one Python object per match — plus, on
non-ASCII text, the cost of materialising UTF-8.

## Where the distance comes from

![Per-group ratios](img/rebar_groups.png)

| group | rurex | `re` |
|---|---:|---:|
| `01-literal` | ×2.74 | ×10.24 |
| `02-literal-alternate` | ×1.58 | ×22.35 |
| `03-date` | ×4.77 | ×13.37 |
| `04-ruff-noqa` | ×5.02 | ×9.29 |
| `05-lexer-veryl` | ×2.28 | ×3.86 |
| `06-cloud-flare-redos` | ×9.72 | ×815.09 |
| `07-unicode-character-data` | ×2.72 | ×4.94 |
| `08-words` | ×1.55 | ×1.40 |
| `09-aws-keys` | ×3.53 | ×5.10 |
| `10-bounded-repeat` | ×1.14 | ×2.22 |
| `11-unstructured-to-json` | ×1.28 | ×1.80 |
| `12-dictionary` | ×1.03 | ×108.70 |
| `14-quadratic` | ×1.66 | ×4.52 |

`re` wins 10 of the 44 shared benchmarks, and they fall into
three groups:

- **Compile time** (3 points): `re`'s parser is
  cheaper than building the automata for these patterns. rurex parses an
  accepted pattern once (ADR-0015), so what remains is the engine's own
  compile: `03-date/compile-unicode` costs
  5.70 ms against `re`'s
  3.71 ms,
  while `rust/regex` itself takes
  5.70 ms;
  `10-bounded-repeat/compile-capitals` costs
  69.57 µs against `re`'s
  28.48 µs, engine
  61.74 µs.
- **Match-heavy iteration** (1 point):
  rebar's counting models call `m.group(0)` per match, so a haystack with
  tens of thousands of matches measures the Python object per match plus
  the accessor call. In
  `08-words/all-english` (an ASCII haystack in text mode, so no transcode,
  with 56,601 matches) rurex
  takes 2.55 ms against
  2.25 ms for `re` and
  695.43 µs for the engine alone: about
  45 ns per
  match for rurex, 40 for `re`, and
  12 for the engine's own loop. The
  object is a `Match` per match (ADR-0016); the rest is the per-call
  boundary.
- **Unicode word boundaries on non-ASCII text** (2 points,
  `all-russian` and `long-russian`): the engine itself is slower than `re` here — `rust/regex`
  takes 6.08 ms against `re`'s
  2.95 ms — because its fast DFA cannot handle a
  Unicode-aware `\b` on non-ASCII bytes and falls back to a slower engine.
  rurex adds its per-match objects on top (8.90 ms
  total); its transcode of this 130 KB haystack is about 65 µs, under 1% of
  the call. Beating `re` there is not a binding task.
- **Near parity scans** (4 points): within 15% of each other, with
  `re` marginally ahead.

rurex wins the other 34 points, sometimes by four orders of
magnitude: `re` is a backtracker, and rebar's dictionary and ReDoS
benchmarks make that visible.

| benchmark where `re` wins | rurex | `re` | `re` is |
|---|---:|---:|---:|
| `01-literal/sherlock-ru` | 227.69 µs | 225.47 µs | 1.01× faster |
| `03-date/compile-unicode` | 5.70 ms | 3.71 ms | 1.54× faster |
| `05-lexer-veryl/single` | 83.67 ms | 82.76 ms | 1.01× faster |
| `08-words/all-english` | 2.55 ms | 2.25 ms | 1.13× faster |
| `08-words/all-russian` | 8.90 ms | 2.95 ms | 3.02× faster |
| `08-words/long-russian` | 3.46 ms | 1.26 ms | 2.75× faster |
| `09-aws-keys/quick` | 170.54 ms | 151.99 ms | 1.12× faster |
| `10-bounded-repeat/compile-capitals` | 69.57 µs | 28.48 µs | 2.44× faster |
| `10-bounded-repeat/compile-context` | 55.90 µs | 46.24 µs | 1.21× faster |
| `11-unstructured-to-json/extract` | 295.99 µs | 194.16 µs | 1.52× faster |


## Every shared benchmark

![Per-benchmark comparison](img/rebar_scatter.png)

| group | benchmark | rurex (str) | `re` | rust/regex | fastest |
|---|---|---:|---:|---:|---|
| `01-literal` | `sherlock-casei-en`| 140.88 µs| 2.74 ms| 85.71 µs| rust/regex |
| `01-literal` | `sherlock-casei-ru`| 418.26 µs| 3.25 ms| 190.15 µs| rust/regex |
| `01-literal` | `sherlock-en`| 57.10 µs| 256.50 µs| 25.19 µs| rust/regex |
| `01-literal` | `sherlock-ru`| 227.69 µs| 225.47 µs| 45.38 µs| rust/regex |
| `01-literal` | `sherlock-zh`| 68.02 µs| 73.22 µs| 18.01 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-en`| 380.91 µs| 17.34 ms| 309.60 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-ru`| 1.34 ms| 26.51 ms| 1.04 ms| rust/regex |
| `02-literal-alternate` | `sherlock-en`| 120.72 µs| 1.54 ms| 75.73 µs| rust/regex |
| `02-literal-alternate` | `sherlock-ru`| 469.79 µs| 3.61 ms| 250.85 µs| rust/regex |
| `02-literal-alternate` | `sherlock-zh`| 127.37 µs| 805.50 µs| 60.29 µs| rust/regex |
| `03-date` | `ascii`| 38.49 ms| 240.01 ms| 1.95 ms| rust/regex |
| `03-date` | `compile-ascii`| 1.68 ms| 3.46 ms| 1.47 ms| rust/regex |
| `03-date` | `compile-unicode`| 5.70 ms| 3.71 ms| 5.70 ms| stdlib re |
| `03-date` | `unicode`| 45.50 ms| 335.25 ms| 1.98 ms| rust/regex |
| `04-ruff-noqa` | `compile-real`| 66.77 µs| 70.66 µs| 63.00 µs| rust/regex |
| `04-ruff-noqa` | `real`| 215.78 ms| 1.11 s| 19.41 ms| rust/regex |
| `04-ruff-noqa` | `tweaked`| 218.87 ms| 254.65 ms| 20.36 ms| rust/regex |
| `05-lexer-veryl` | `compile-single`| 297.55 µs| 862.63 µs| 275.34 µs| rust/regex |
| `05-lexer-veryl` | `multi`| —| —| 2.12 ms| rust/regex |
| `05-lexer-veryl` | `single`| 83.67 ms| 82.76 ms| 17.37 ms| rust/regex |
| `06-cloud-flare-redos` | `original`| 2.65 µs| 4.97 µs| 193 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-long`| 827 ns| 26.71 ms| 117 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-short`| 492 ns| 4.79 µs| 52 ns| rust/regex |
| `07-unicode-character-data` | `compile`| 32.20 µs| 99.31 µs| 28.99 µs| rust/regex |
| `07-unicode-character-data` | `parse-line`| 35.87 ms| 38.32 ms| 5.37 ms| rust/regex |
| `08-words` | `all-english`| 2.55 ms| 2.25 ms| 695.43 µs| rust/regex |
| `08-words` | `all-russian`| 8.90 ms| 2.95 ms| 6.08 ms| stdlib re |
| `08-words` | `long-english`| 107.92 µs| 673.95 µs| 100.96 µs| rust/regex |
| `08-words` | `long-russian`| 3.46 ms| 1.26 ms| 3.47 ms| stdlib re |
| `09-aws-keys` | `compile-full`| 100.64 µs| 170.27 µs| 93.71 µs| rust/regex |
| `09-aws-keys` | `compile-quick`| 18.99 µs| 39.57 µs| 15.66 µs| rust/regex |
| `09-aws-keys` | `full`| 211.36 ms| 293.46 ms| 17.65 ms| rust/regex |
| `09-aws-keys` | `quick`| 170.54 ms| 151.99 ms| 17.15 ms| rust/regex |
| `10-bounded-repeat` | `capitals`| 9.33 ms| 105.87 ms| 9.31 ms| rust/regex |
| `10-bounded-repeat` | `compile-capitals`| 69.57 µs| 28.48 µs| 61.74 µs| stdlib re |
| `10-bounded-repeat` | `compile-context`| 55.90 µs| 46.24 µs| 60.04 µs| stdlib re |
| `10-bounded-repeat` | `context`| 78.90 ms| 105.92 ms| 75.18 ms| rust/regex |
| `10-bounded-repeat` | `letters-en`| 388.08 µs| 2.09 ms| 221.64 µs| rust/regex |
| `10-bounded-repeat` | `letters-ru`| —| —| 405.27 µs| rust/regex |
| `11-unstructured-to-json` | `compile`| 24.39 µs| 73.84 µs| 20.34 µs| rust/regex |
| `11-unstructured-to-json` | `extract`| 295.99 µs| 194.16 µs| 217.98 µs| stdlib re |
| `12-dictionary` | `compile-multi`| —| —| 14.82 ms| rust/regex |
| `12-dictionary` | `compile-single`| 8.79 ms| 24.52 ms| 8.60 ms| rust/regex |
| `12-dictionary` | `multi`| —| —| 357.59 µs| rust/regex |
| `12-dictionary` | `single`| 92.34 µs| 370.85 ms| 89.48 µs| rust/regex |
| `13-noseyparker` | `compile-multi`| —| —| 2.72 ms| rust/regex |
| `13-noseyparker` | `compile-single`| —| —| 2.27 ms| rust/regex |
| `13-noseyparker` | `multi`| —| —| 329.20 ms| rust/regex |
| `13-noseyparker` | `single`| —| —| 265.37 ms| rust/regex |
| `14-quadratic` | `10x`| 701.26 µs| 2.16 ms| 621.85 µs| rust/regex |
| `14-quadratic` | `1x`| 14.25 µs| 34.51 µs| 5.89 µs| rust/regex |
| `14-quadratic` | `2x`| 40.75 µs| 110.13 µs| 24.23 µs| rust/regex |

## Text mode versus byte mode, identical input

`dictionary/search/english-15` runs in Unicode mode over an ASCII haystack, so the two
representations are interchangeable for the match count. rurex's text
path borrows ASCII strings instead of transcoding them, which is why the
two modes are within a couple of percent here; what remains against the
Rust engine is the Python boundary.

![Text vs bytes](img/rebar_modes.png)

| engine | throughput |
|---|---:|
| `rust/regex` | 686 MB/s |
| `python/rurex-bytes` | 679 MB/s |
| `python/rurex` | 663 MB/s |
| `python/re` | 167 KB/s |

The non-ASCII case is the interesting one, and rebar cannot measure it
without changing match counts: on a Russian or Chinese haystack the byte
engine matches ASCII word characters only. That comparison lives in
[`telegram-benchmark.md`](telegram-benchmark.md), where the text row pays
one UCS→UTF-8 encode per post and the byte row pays none.

## What this does and does not say

- rebar's counting models iterate `finditer`, so one Python object per
  match is inside the measurement. That is what a `finditer` user pays,
  but it is **not** what `findall` pays: rurex's `findall` path builds
  strings directly, with no `Match` objects, and rebar has no model for
  it. The per-match numbers here therefore overstate the cost for
  `findall`-shaped workloads.
- Most rebar benchmarks set `unicode = false`, i.e. they hand every
  engine bytes. Those points measure the path with no transcoding at
  all; the transcode appears only in the Unicode-mode benchmarks
  (`03-date/unicode`, the Russian and Chinese literals and words).
- The ranking is a geometric mean across heterogeneous tasks. Read the
  per-group and per-benchmark tables for anything decision-shaped; the
  mean is a summary, not a measurement of your workload.

## Reproducing

`benchmarks/rebar/README.md` holds the runner, the engine registration and
the full invocation. The short version, on the benchmark host:

```sh
python3 benchmarks/rebar_run.py          # measure, verify, then rebuild this page
```

which takes the core lease, pins to CPU 0, runs rebar for the four
engines, and calls `benchmarks/rebar_charts.py` only if every point
passed.
