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
| rurex | 0.2.0, local wheel built from the work tree (commit 5425fab + capture-location reuse), sha256 27e2ffee |
| Opponents | stdlib `re`, `rust/regex` (the engine rurex embeds) |
| Checks | every point verified with `rebar measure --test` before timing; no failed points in the data |

| Raw results | sha256 |
|---|---|
| `curated.csv` | f9e20536f5ac3a87… |
| `dict.csv` | a73860be04188240… |

## The ranking

![Geometric mean ranking](img/rebar_rank.png)

| engine | score | benchmarks |
|---|---:|---:|
| `rust/regex` | ×1.08 | 44 |
| `python/rurex` | ×2.59 | 44 |
| `python/re` | ×9.75 | 44 |

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
| `01-literal` | ×2.73 | ×10.29 |
| `02-literal-alternate` | ×1.57 | ×21.80 |
| `03-date` | ×4.68 | ×13.18 |
| `04-ruff-noqa` | ×4.98 | ×9.11 |
| `05-lexer-veryl` | ×2.26 | ×3.78 |
| `06-cloud-flare-redos` | ×9.62 | ×821.65 |
| `07-unicode-character-data` | ×2.79 | ×4.93 |
| `08-words` | ×1.54 | ×1.39 |
| `09-aws-keys` | ×3.69 | ×5.11 |
| `10-bounded-repeat` | ×1.16 | ×2.21 |
| `11-unstructured-to-json` | ×1.32 | ×1.83 |
| `12-dictionary` | ×1.00 | ×108.63 |
| `14-quadratic` | ×1.63 | ×4.46 |

`re` wins 10 of the 44 shared benchmarks, and they fall into
three groups:

- **Compile time** (3 points): `re`'s parser is
  cheaper than building the automata for these patterns. rurex parses an
  accepted pattern once (ADR-0015), so what remains is the engine's own
  compile: `03-date/compile-unicode` costs
  5.67 ms against `re`'s
  3.72 ms,
  while `rust/regex` itself takes
  5.73 ms;
  `10-bounded-repeat/compile-capitals` costs
  72.64 µs against `re`'s
  27.48 µs, engine
  61.93 µs.
- **Match-heavy iteration** (1 point):
  rebar's counting models call `m.group(0)` per match, so a haystack with
  tens of thousands of matches measures the Python object per match plus
  the accessor call. In
  `08-words/all-english` (an ASCII haystack in text mode, so no transcode,
  with 56,601 matches) rurex
  takes 2.54 ms against
  2.25 ms for `re` and
  712.14 µs for the engine alone: about
  45 ns per
  match for rurex, 40 for `re`, and
  13 for the engine's own loop. The
  object is a `Match` per match (ADR-0016); the rest is the per-call
  boundary.
- **Unicode word boundaries on non-ASCII text** (2 points,
  `all-russian` and `long-russian`): the engine itself is slower than `re` here — `rust/regex`
  takes 6.31 ms against `re`'s
  2.98 ms — because its fast DFA cannot handle a
  Unicode-aware `\b` on non-ASCII bytes and falls back to a slower engine.
  rurex adds its per-match objects on top (8.99 ms
  total); its transcode of this 130 KB haystack is about 65 µs, under 1% of
  the call. Beating `re` there is not a binding task.
- **Near parity scans** (4 points): within 15% of each other, with
  `re` marginally ahead.

rurex wins the other 34 points, sometimes by four orders of
magnitude: `re` is a backtracker, and rebar's dictionary and ReDoS
benchmarks make that visible.

| benchmark where `re` wins | rurex | `re` | `re` is |
|---|---:|---:|---:|
| `01-literal/sherlock-ru` | 227.18 µs | 225.38 µs | 1.01× faster |
| `03-date/compile-unicode` | 5.67 ms | 3.72 ms | 1.52× faster |
| `05-lexer-veryl/single` | 83.82 ms | 80.50 ms | 1.04× faster |
| `08-words/all-english` | 2.54 ms | 2.25 ms | 1.13× faster |
| `08-words/all-russian` | 8.99 ms | 2.98 ms | 3.02× faster |
| `08-words/long-russian` | 3.52 ms | 1.26 ms | 2.79× faster |
| `09-aws-keys/quick` | 168.21 ms | 151.19 ms | 1.11× faster |
| `10-bounded-repeat/compile-capitals` | 72.64 µs | 27.48 µs | 2.64× faster |
| `10-bounded-repeat/compile-context` | 59.51 µs | 46.72 µs | 1.27× faster |
| `11-unstructured-to-json/extract` | 296.90 µs | 195.16 µs | 1.52× faster |


## Every shared benchmark

![Per-benchmark comparison](img/rebar_scatter.png)

| group | benchmark | rurex (str) | `re` | rust/regex | fastest |
|---|---|---:|---:|---:|---|
| `01-literal` | `sherlock-casei-en`| 137.72 µs| 2.74 ms| 85.75 µs| rust/regex |
| `01-literal` | `sherlock-casei-ru`| 418.28 µs| 3.27 ms| 191.90 µs| rust/regex |
| `01-literal` | `sherlock-en`| 55.88 µs| 255.69 µs| 25.17 µs| rust/regex |
| `01-literal` | `sherlock-ru`| 227.18 µs| 225.38 µs| 45.61 µs| rust/regex |
| `01-literal` | `sherlock-zh`| 68.30 µs| 73.11 µs| 17.31 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-en`| 380.77 µs| 17.02 ms| 309.59 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-ru`| 1.33 ms| 26.64 ms| 1.03 ms| rust/regex |
| `02-literal-alternate` | `sherlock-en`| 122.63 µs| 1.54 ms| 81.09 µs| rust/regex |
| `02-literal-alternate` | `sherlock-ru`| 463.48 µs| 3.61 ms| 247.14 µs| rust/regex |
| `02-literal-alternate` | `sherlock-zh`| 126.94 µs| 752.25 µs| 60.24 µs| rust/regex |
| `03-date` | `ascii`| 38.58 ms| 240.26 ms| 2.05 ms| rust/regex |
| `03-date` | `compile-ascii`| 1.65 ms| 3.47 ms| 1.47 ms| rust/regex |
| `03-date` | `compile-unicode`| 5.67 ms| 3.72 ms| 5.73 ms| stdlib re |
| `03-date` | `unicode`| 45.54 ms| 333.06 ms| 1.98 ms| rust/regex |
| `04-ruff-noqa` | `compile-real`| 66.69 µs| 68.96 µs| 65.78 µs| rust/regex |
| `04-ruff-noqa` | `real`| 215.73 ms| 1.11 s| 19.34 ms| rust/regex |
| `04-ruff-noqa` | `tweaked`| 218.89 ms| 251.58 ms| 20.01 ms| rust/regex |
| `05-lexer-veryl` | `compile-single`| 295.02 µs| 859.31 µs| 279.53 µs| rust/regex |
| `05-lexer-veryl` | `multi`| —| —| 2.26 ms| rust/regex |
| `05-lexer-veryl` | `single`| 83.82 ms| 80.50 ms| 17.28 ms| rust/regex |
| `06-cloud-flare-redos` | `original`| 2.64 µs| 5.12 µs| 192 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-long`| 798 ns| 26.41 ms| 115 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-short`| 485 ns| 4.71 µs| 52 ns| rust/regex |
| `07-unicode-character-data` | `compile`| 34.23 µs| 98.95 µs| 29.10 µs| rust/regex |
| `07-unicode-character-data` | `parse-line`| 35.83 ms| 38.54 ms| 5.40 ms| rust/regex |
| `08-words` | `all-english`| 2.54 ms| 2.25 ms| 712.14 µs| rust/regex |
| `08-words` | `all-russian`| 8.99 ms| 2.98 ms| 6.31 ms| stdlib re |
| `08-words` | `long-english`| 107.92 µs| 672.97 µs| 98.88 µs| rust/regex |
| `08-words` | `long-russian`| 3.52 ms| 1.26 ms| 3.45 ms| stdlib re |
| `09-aws-keys` | `compile-full`| 106.79 µs| 170.08 µs| 92.47 µs| rust/regex |
| `09-aws-keys` | `compile-quick`| 20.64 µs| 39.23 µs| 15.80 µs| rust/regex |
| `09-aws-keys` | `full`| 217.80 ms| 294.43 ms| 17.60 ms| rust/regex |
| `09-aws-keys` | `quick`| 168.21 ms| 151.19 ms| 16.92 ms| rust/regex |
| `10-bounded-repeat` | `capitals`| 9.32 ms| 105.88 ms| 9.29 ms| rust/regex |
| `10-bounded-repeat` | `compile-capitals`| 72.64 µs| 27.48 µs| 61.93 µs| stdlib re |
| `10-bounded-repeat` | `compile-context`| 59.51 µs| 46.72 µs| 59.89 µs| stdlib re |
| `10-bounded-repeat` | `context`| 77.80 ms| 105.89 ms| 75.00 ms| rust/regex |
| `10-bounded-repeat` | `letters-en`| 387.04 µs| 2.10 ms| 223.43 µs| rust/regex |
| `10-bounded-repeat` | `letters-ru`| —| —| 406.22 µs| rust/regex |
| `11-unstructured-to-json` | `compile`| 25.42 µs| 74.19 µs| 20.20 µs| rust/regex |
| `11-unstructured-to-json` | `extract`| 296.90 µs| 195.16 µs| 215.03 µs| stdlib re |
| `12-dictionary` | `compile-multi`| —| —| 14.81 ms| rust/regex |
| `12-dictionary` | `compile-single`| 8.52 ms| 24.62 ms| 8.63 ms| rurex (str) |
| `12-dictionary` | `multi`| —| —| 358.53 µs| rust/regex |
| `12-dictionary` | `single`| 90.23 µs| 369.82 ms| 89.41 µs| rust/regex |
| `13-noseyparker` | `compile-multi`| —| —| 2.72 ms| rust/regex |
| `13-noseyparker` | `compile-single`| —| —| 2.39 ms| rust/regex |
| `13-noseyparker` | `multi`| —| —| 328.88 ms| rust/regex |
| `13-noseyparker` | `single`| —| —| 264.65 ms| rust/regex |
| `14-quadratic` | `10x`| 701.12 µs| 2.17 ms| 622.07 µs| rust/regex |
| `14-quadratic` | `1x`| 14.10 µs| 34.34 µs| 5.94 µs| rust/regex |
| `14-quadratic` | `2x`| 40.36 µs| 108.75 µs| 24.80 µs| rust/regex |

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
| `python/rurex` | 666 MB/s |
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
