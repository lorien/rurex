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
| Date | 2026-10-20 |
| Host | `regexus`, 13th Gen Intel(R) Core(TM) i5-13500 |
| Isolation | `/tmp/core4.lock` lease (cpus [0, 2, 4, 6]), pinned to CPU 0, governor `performance` |
| rebar | 0.1.0 (curated set, `--max-time 3s --max-iters 10`) |
| rurex | 0.2.0, local wheel built from the work tree (commit e0bdfa8 + per-match work), sha256 8bba10b5 |
| Opponents | stdlib `re`, `rust/regex` (the engine rurex embeds) |
| Checks | every point verified with `rebar measure --test` before timing; no failed points in the data |

| Raw results | sha256 |
|---|---|
| `curated.csv` | 33450f9ef4cd74ab… |
| `dict.csv` | 1a116cee5387f9a7… |

## The ranking

![Geometric mean ranking](img/rebar_rank.png)

| engine | score | benchmarks |
|---|---:|---:|
| `rust/regex` | ×1.08 | 44 |
| `python/rurex` | ×2.65 | 44 |
| `python/re` | ×9.72 | 44 |

rurex sits between the engine it embeds and the module it replaces: about
**3.7× faster than `re`** on average, and
about **2.7× off the Rust engine**. The distance to the engine is
not the scanner: the two run the same automata. It is the Python
boundary — argument handling and one Python object per match — plus, on
non-ASCII text, the cost of materialising UTF-8.

## Where the distance comes from

![Per-group ratios](img/rebar_groups.png)

| group | rurex | `re` |
|---|---:|---:|
| `01-literal` | ×2.74 | ×9.96 |
| `02-literal-alternate` | ×1.64 | ×22.07 |
| `03-date` | ×4.76 | ×13.32 |
| `04-ruff-noqa` | ×5.25 | ×9.21 |
| `05-lexer-veryl` | ×2.29 | ×3.74 |
| `06-cloud-flare-redos` | ×9.68 | ×790.52 |
| `07-unicode-character-data` | ×2.82 | ×4.94 |
| `08-words` | ×1.62 | ×1.41 |
| `09-aws-keys` | ×3.79 | ×5.16 |
| `10-bounded-repeat` | ×1.17 | ×2.20 |
| `11-unstructured-to-json` | ×1.32 | ×1.84 |
| `12-dictionary` | ×1.00 | ×105.77 |
| `14-quadratic` | ×1.75 | ×4.48 |

`re` wins 10 of the 44 shared benchmarks, and they fall into
three groups:

- **Compile time** (3 points): `re`'s parser is
  cheaper than building the automata for these patterns. rurex parses an
  accepted pattern once (ADR-0015), so what remains is the engine's own
  compile: `03-date/compile-unicode` costs
  5.66 ms against `re`'s
  3.69 ms,
  while `rust/regex` itself takes
  5.72 ms;
  `10-bounded-repeat/compile-capitals` costs
  72.17 µs against `re`'s
  27.59 µs, engine
  61.23 µs.
- **Match-heavy iteration** (1 point):
  rebar's counting models call `m.group(0)` per match, so a haystack with
  tens of thousands of matches measures the Python object per match plus
  the accessor call. In
  `08-words/all-english` (an ASCII haystack in text mode, so no transcode,
  with 56,601 matches) rurex
  takes 2.76 ms against
  2.25 ms for `re` and
  695.72 µs for the engine alone: about
  49 ns per
  match for rurex, 40 for `re`, and
  12 for the engine's own loop. The
  object is a `Match` per match (ADR-0016); the rest is the per-call
  boundary.
- **Unicode word boundaries on non-ASCII text** (2 points,
  `all-russian` and `long-russian`): the engine itself is slower than `re` here — `rust/regex`
  takes 5.99 ms against `re`'s
  2.95 ms — because its fast DFA cannot handle a
  Unicode-aware `\b` on non-ASCII bytes and falls back to a slower engine.
  rurex adds its per-match objects on top (9.20 ms
  total); its transcode of this 130 KB haystack is about 65 µs, under 1% of
  the call. Beating `re` there is not a binding task.
- **Near parity scans** (4 points): within 15% of each other, with
  `re` marginally ahead.

rurex wins the other 34 points, sometimes by four orders of
magnitude: `re` is a backtracker, and rebar's dictionary and ReDoS
benchmarks make that visible.

| benchmark where `re` wins | rurex | `re` | `re` is |
|---|---:|---:|---:|
| `01-literal/sherlock-ru` | 231.64 µs | 225.04 µs | 1.03× faster |
| `03-date/compile-unicode` | 5.66 ms | 3.69 ms | 1.53× faster |
| `05-lexer-veryl/single` | 86.99 ms | 80.96 ms | 1.07× faster |
| `08-words/all-english` | 2.76 ms | 2.25 ms | 1.23× faster |
| `08-words/all-russian` | 9.20 ms | 2.95 ms | 3.12× faster |
| `08-words/long-russian` | 3.51 ms | 1.26 ms | 2.79× faster |
| `09-aws-keys/quick` | 177.48 ms | 151.68 ms | 1.17× faster |
| `10-bounded-repeat/compile-capitals` | 72.17 µs | 27.59 µs | 2.62× faster |
| `10-bounded-repeat/compile-context` | 59.53 µs | 45.74 µs | 1.30× faster |
| `11-unstructured-to-json/extract` | 300.91 µs | 201.84 µs | 1.49× faster |


## Every shared benchmark

![Per-benchmark comparison](img/rebar_scatter.png)

| group | benchmark | rurex (str) | `re` | rust/regex | fastest |
|---|---|---:|---:|---:|---|
| `01-literal` | `sherlock-casei-en`| 142.81 µs| 2.81 ms| 85.41 µs| rust/regex |
| `01-literal` | `sherlock-casei-ru`| 429.70 µs| 3.26 ms| 194.03 µs| rust/regex |
| `01-literal` | `sherlock-en`| 62.35 µs| 256.37 µs| 25.85 µs| rust/regex |
| `01-literal` | `sherlock-ru`| 231.64 µs| 225.04 µs| 45.70 µs| rust/regex |
| `01-literal` | `sherlock-zh`| 68.76 µs| 73.04 µs| 20.13 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-en`| 393.88 µs| 17.25 ms| 309.67 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-ru`| 1.35 ms| 26.46 ms| 1.04 ms| rust/regex |
| `02-literal-alternate` | `sherlock-en`| 128.50 µs| 1.54 ms| 76.02 µs| rust/regex |
| `02-literal-alternate` | `sherlock-ru`| 488.08 µs| 3.60 ms| 246.36 µs| rust/regex |
| `02-literal-alternate` | `sherlock-zh`| 129.75 µs| 752.19 µs| 60.24 µs| rust/regex |
| `03-date` | `ascii`| 38.75 ms| 241.08 ms| 1.94 ms| rust/regex |
| `03-date` | `compile-ascii`| 1.65 ms| 3.44 ms| 1.47 ms| rust/regex |
| `03-date` | `compile-unicode`| 5.66 ms| 3.69 ms| 5.72 ms| stdlib re |
| `03-date` | `unicode`| 46.01 ms| 334.25 ms| 1.99 ms| rust/regex |
| `04-ruff-noqa` | `compile-real`| 67.78 µs| 68.75 µs| 63.46 µs| rust/regex |
| `04-ruff-noqa` | `real`| 230.78 ms| 1.12 s| 19.49 ms| rust/regex |
| `04-ruff-noqa` | `tweaked`| 229.29 ms| 251.86 ms| 20.04 ms| rust/regex |
| `05-lexer-veryl` | `compile-single`| 297.20 µs| 855.07 µs| 276.19 µs| rust/regex |
| `05-lexer-veryl` | `multi`| —| —| 2.17 ms| rust/regex |
| `05-lexer-veryl` | `single`| 86.99 ms| 80.96 ms| 17.89 ms| rust/regex |
| `06-cloud-flare-redos` | `original`| 2.68 µs| 4.87 µs| 193 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-long`| 815 ns| 26.56 ms| 118 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-short`| 501 ns| 4.61 µs| 53 ns| rust/regex |
| `07-unicode-character-data` | `compile`| 33.16 µs| 99.45 µs| 28.43 µs| rust/regex |
| `07-unicode-character-data` | `parse-line`| 36.93 ms| 37.87 ms| 5.42 ms| rust/regex |
| `08-words` | `all-english`| 2.76 ms| 2.25 ms| 695.72 µs| rust/regex |
| `08-words` | `all-russian`| 9.20 ms| 2.95 ms| 5.99 ms| stdlib re |
| `08-words` | `long-english`| 109.20 µs| 670.06 µs| 98.83 µs| rust/regex |
| `08-words` | `long-russian`| 3.51 ms| 1.26 ms| 3.45 ms| stdlib re |
| `09-aws-keys` | `compile-full`| 107.17 µs| 165.80 µs| 91.45 µs| rust/regex |
| `09-aws-keys` | `compile-quick`| 19.80 µs| 39.42 µs| 15.03 µs| rust/regex |
| `09-aws-keys` | `full`| 225.28 ms| 292.98 ms| 17.63 ms| rust/regex |
| `09-aws-keys` | `quick`| 177.48 ms| 151.68 ms| 16.88 ms| rust/regex |
| `10-bounded-repeat` | `capitals`| 9.30 ms| 105.91 ms| 9.30 ms| rurex (str) |
| `10-bounded-repeat` | `compile-capitals`| 72.17 µs| 27.59 µs| 61.23 µs| stdlib re |
| `10-bounded-repeat` | `compile-context`| 59.53 µs| 45.74 µs| 61.60 µs| stdlib re |
| `10-bounded-repeat` | `context`| 77.79 ms| 105.81 ms| 75.15 ms| rust/regex |
| `10-bounded-repeat` | `letters-en`| 415.07 µs| 2.10 ms| 219.57 µs| rust/regex |
| `10-bounded-repeat` | `letters-ru`| —| —| 400.47 µs| rust/regex |
| `11-unstructured-to-json` | `compile`| 25.68 µs| 74.11 µs| 20.50 µs| rust/regex |
| `11-unstructured-to-json` | `extract`| 300.91 µs| 201.84 µs| 215.50 µs| stdlib re |
| `12-dictionary` | `compile-multi`| —| —| 14.81 ms| rust/regex |
| `12-dictionary` | `compile-single`| 9.03 ms| 24.68 ms| 9.09 ms| rurex (str) |
| `12-dictionary` | `multi`| —| —| 360.35 µs| rust/regex |
| `12-dictionary` | `single`| 90.32 µs| 368.31 ms| 89.39 µs| rust/regex |
| `13-noseyparker` | `compile-multi`| —| —| 2.72 ms| rust/regex |
| `13-noseyparker` | `compile-single`| —| —| 2.26 ms| rust/regex |
| `13-noseyparker` | `multi`| —| —| 327.97 ms| rust/regex |
| `13-noseyparker` | `single`| —| —| 264.99 ms| rust/regex |
| `14-quadratic` | `10x`| 715.01 µs| 2.13 ms| 622.03 µs| rust/regex |
| `14-quadratic` | `1x`| 15.27 µs| 34.34 µs| 5.89 µs| rust/regex |
| `14-quadratic` | `2x`| 43.96 µs| 109.10 µs| 24.25 µs| rust/regex |

## Text mode versus byte mode, identical input

`dictionary/search/english-15` runs in Unicode mode over an ASCII haystack, so the two
representations are interchangeable for the match count. rurex's text
path borrows ASCII strings instead of transcoding them, which is why the
two modes are within a couple of percent here; what remains against the
Rust engine is the Python boundary.

![Text vs bytes](img/rebar_modes.png)

| engine | throughput |
|---|---:|
| `rust/regex` | 687 MB/s |
| `python/rurex-bytes` | 677 MB/s |
| `python/rurex` | 663 MB/s |
| `python/re` | 166 KB/s |

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
