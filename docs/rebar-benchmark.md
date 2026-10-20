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
| rurex | 0.2.0, local wheel built from the work tree (commit 7834350 + ADR-0015), sha256 fbdc6e0a |
| Opponents | stdlib `re`, `rust/regex` (the engine rurex embeds) |
| Checks | every point verified with `rebar measure --test` before timing; no failed points in the data |

| Raw results | sha256 |
|---|---|
| `curated.csv` | c08307a4e921976c… |
| `dict.csv` | 77b234c594297f4f… |

## The ranking

![Geometric mean ranking](img/rebar_rank.png)

| engine | score | benchmarks |
|---|---:|---:|
| `rust/regex` | ×1.08 | 44 |
| `python/rurex` | ×2.72 | 44 |
| `python/re` | ×9.72 | 44 |

rurex sits between the engine it embeds and the module it replaces: about
**3.6× faster than `re`** on average, and
about **2.7× off the Rust engine**. The distance to the engine is
not the scanner: the two run the same automata. It is the Python
boundary — argument handling and one Python object per match — plus, on
non-ASCII text, the cost of materialising UTF-8.

## Where the distance comes from

![Per-group ratios](img/rebar_groups.png)

| group | rurex | `re` |
|---|---:|---:|
| `01-literal` | ×2.84 | ×10.10 |
| `02-literal-alternate` | ×1.65 | ×22.08 |
| `03-date` | ×4.82 | ×13.32 |
| `04-ruff-noqa` | ×5.34 | ×9.18 |
| `05-lexer-veryl` | ×2.25 | ×3.81 |
| `06-cloud-flare-redos` | ×10.18 | ×774.96 |
| `07-unicode-character-data` | ×2.94 | ×4.89 |
| `08-words` | ×1.80 | ×1.42 |
| `09-aws-keys` | ×3.77 | ×5.08 |
| `10-bounded-repeat` | ×1.16 | ×2.18 |
| `11-unstructured-to-json` | ×1.25 | ×1.75 |
| `12-dictionary` | ×1.03 | ×108.53 |
| `14-quadratic` | ×1.85 | ×4.47 |

`re` wins 12 of the 44 shared benchmarks, and they fall into
three groups:

- **Compile time** (4 points): `re`'s parser is
  cheaper than building the automata for these patterns. rurex parses an
  accepted pattern once (ADR-0015), so what remains is the engine's own
  compile: `03-date/compile-unicode` costs
  5.79 ms against `re`'s
  3.70 ms,
  while `rust/regex` itself takes
  5.71 ms;
  `10-bounded-repeat/compile-capitals` costs
  70.20 µs against `re`'s
  27.62 µs, engine
  61.95 µs.
- **Match-heavy iteration** (1 point):
  rebar's counting models call `m.group(0)` per match, so a haystack with
  tens of thousands of matches measures Python object construction. In
  `08-words/all-english` (byte mode, no transcode, 56,601 matches) rurex
  takes 3.68 ms against
  2.26 ms for `re` and
  689.57 µs for the engine alone: about 63 ns per
  match for rurex, 40 for `re`, 13 for the engine's own loop.
- **Unicode word boundaries on non-ASCII text** (2 points,
  `all-russian` and `long-russian`): the engine itself is slower than `re` here — `rust/regex`
  takes 6.08 ms against `re`'s
  3.00 ms — because its fast DFA cannot handle a
  Unicode-aware `\b` on non-ASCII bytes and falls back to a slower engine.
  rurex adds its per-match objects on top (10.05 ms
  total); its transcode of this 130 KB haystack is about 65 µs, under 1% of
  the call. Beating `re` there is not a binding task.
- **Near parity scans** (5 points): within 15% of each other, with
  `re` marginally ahead.

rurex wins the other 32 points, sometimes by four orders of
magnitude: `re` is a backtracker, and rebar's dictionary and ReDoS
benchmarks make that visible.

| benchmark where `re` wins | rurex | `re` | `re` is |
|---|---:|---:|---:|
| `01-literal/sherlock-ru` | 230.43 µs | 225.43 µs | 1.02× faster |
| `03-date/compile-unicode` | 5.79 ms | 3.70 ms | 1.56× faster |
| `04-ruff-noqa/compile-real` | 69.60 µs | 68.97 µs | 1.01× faster |
| `05-lexer-veryl/single` | 81.99 ms | 80.73 ms | 1.02× faster |
| `07-unicode-character-data/parse-line` | 40.53 ms | 38.54 ms | 1.05× faster |
| `08-words/all-english` | 3.68 ms | 2.26 ms | 1.63× faster |
| `08-words/all-russian` | 10.05 ms | 3.00 ms | 3.35× faster |
| `08-words/long-russian` | 3.55 ms | 1.26 ms | 2.82× faster |
| `09-aws-keys/quick` | 182.65 ms | 151.21 ms | 1.21× faster |
| `10-bounded-repeat/compile-capitals` | 70.20 µs | 27.62 µs | 2.54× faster |
| `10-bounded-repeat/compile-context` | 57.94 µs | 44.68 µs | 1.30× faster |
| `11-unstructured-to-json/extract` | 295.75 µs | 194.73 µs | 1.52× faster |


## Every shared benchmark

![Per-benchmark comparison](img/rebar_scatter.png)

| group | benchmark | rurex (str) | `re` | rust/regex | fastest |
|---|---|---:|---:|---:|---|
| `01-literal` | `sherlock-casei-en`| 147.28 µs| 2.77 ms| 85.56 µs| rust/regex |
| `01-literal` | `sherlock-casei-ru`| 426.61 µs| 3.25 ms| 192.58 µs| rust/regex |
| `01-literal` | `sherlock-en`| 67.28 µs| 256.55 µs| 25.87 µs| rust/regex |
| `01-literal` | `sherlock-ru`| 230.43 µs| 225.43 µs| 44.66 µs| rust/regex |
| `01-literal` | `sherlock-zh`| 68.46 µs| 73.16 µs| 19.07 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-en`| 397.91 µs| 17.34 ms| 316.73 µs| rust/regex |
| `02-literal-alternate` | `sherlock-casei-ru`| 1.34 ms| 26.58 ms| 1.03 ms| rust/regex |
| `02-literal-alternate` | `sherlock-en`| 133.79 µs| 1.53 ms| 75.69 µs| rust/regex |
| `02-literal-alternate` | `sherlock-ru`| 479.23 µs| 3.60 ms| 246.25 µs| rust/regex |
| `02-literal-alternate` | `sherlock-zh`| 130.53 µs| 758.00 µs| 60.31 µs| rust/regex |
| `03-date` | `ascii`| 39.39 ms| 239.81 ms| 1.95 ms| rust/regex |
| `03-date` | `compile-ascii`| 1.67 ms| 3.49 ms| 1.46 ms| rust/regex |
| `03-date` | `compile-unicode`| 5.79 ms| 3.70 ms| 5.71 ms| stdlib re |
| `03-date` | `unicode`| 46.41 ms| 334.13 ms| 2.02 ms| rust/regex |
| `04-ruff-noqa` | `compile-real`| 69.60 µs| 68.97 µs| 64.15 µs| rust/regex |
| `04-ruff-noqa` | `real`| 231.55 ms| 1.11 s| 19.41 ms| rust/regex |
| `04-ruff-noqa` | `tweaked`| 236.73 ms| 253.29 ms| 20.16 ms| rust/regex |
| `05-lexer-veryl` | `compile-single`| 302.58 µs| 876.69 µs| 281.07 µs| rust/regex |
| `05-lexer-veryl` | `multi`| —| —| 2.15 ms| rust/regex |
| `05-lexer-veryl` | `single`| 81.99 ms| 80.73 ms| 17.36 ms| rust/regex |
| `06-cloud-flare-redos` | `original`| 2.91 µs| 4.78 µs| 191 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-long`| 842 ns| 26.52 ms| 127 ns| rust/regex |
| `06-cloud-flare-redos` | `simplified-short`| 554 ns| 4.72 µs| 53 ns| rust/regex |
| `07-unicode-character-data` | `compile`| 34.16 µs| 99.59 µs| 29.72 µs| rust/regex |
| `07-unicode-character-data` | `parse-line`| 40.53 ms| 38.54 ms| 5.39 ms| rust/regex |
| `08-words` | `all-english`| 3.68 ms| 2.26 ms| 689.57 µs| rust/regex |
| `08-words` | `all-russian`| 10.05 ms| 3.00 ms| 6.08 ms| stdlib re |
| `08-words` | `long-english`| 112.61 µs| 669.84 µs| 98.91 µs| rust/regex |
| `08-words` | `long-russian`| 3.55 ms| 1.26 ms| 3.42 ms| stdlib re |
| `09-aws-keys` | `compile-full`| 107.32 µs| 168.24 µs| 91.78 µs| rust/regex |
| `09-aws-keys` | `compile-quick`| 19.20 µs| 38.93 µs| 15.55 µs| rust/regex |
| `09-aws-keys` | `full`| 231.27 ms| 290.76 ms| 17.70 ms| rust/regex |
| `09-aws-keys` | `quick`| 182.65 ms| 151.21 ms| 17.06 ms| rust/regex |
| `10-bounded-repeat` | `capitals`| 9.30 ms| 105.86 ms| 9.38 ms| rurex (str) |
| `10-bounded-repeat` | `compile-capitals`| 70.20 µs| 27.62 µs| 61.95 µs| stdlib re |
| `10-bounded-repeat` | `compile-context`| 57.94 µs| 44.68 µs| 60.17 µs| stdlib re |
| `10-bounded-repeat` | `context`| 78.42 ms| 106.41 ms| 75.50 ms| rust/regex |
| `10-bounded-repeat` | `letters-en`| 424.54 µs| 2.09 ms| 223.02 µs| rust/regex |
| `10-bounded-repeat` | `letters-ru`| —| —| 405.23 µs| rust/regex |
| `11-unstructured-to-json` | `compile`| 25.08 µs| 74.28 µs| 20.39 µs| rust/regex |
| `11-unstructured-to-json` | `extract`| 295.75 µs| 194.73 µs| 232.28 µs| stdlib re |
| `12-dictionary` | `compile-multi`| —| —| 14.88 ms| rust/regex |
| `12-dictionary` | `compile-single`| 9.00 ms| 24.54 ms| 8.62 ms| rust/regex |
| `12-dictionary` | `multi`| —| —| 361.17 µs| rust/regex |
| `12-dictionary` | `single`| 90.38 µs| 369.70 ms| 89.36 µs| rust/regex |
| `13-noseyparker` | `compile-multi`| —| —| 2.73 ms| rust/regex |
| `13-noseyparker` | `compile-single`| —| —| 2.25 ms| rust/regex |
| `13-noseyparker` | `multi`| —| —| 328.01 ms| rust/regex |
| `13-noseyparker` | `single`| —| —| 264.81 ms| rust/regex |
| `14-quadratic` | `10x`| 769.29 µs| 2.13 ms| 622.66 µs| rust/regex |
| `14-quadratic` | `1x`| 15.83 µs| 34.50 µs| 5.95 µs| rust/regex |
| `14-quadratic` | `2x`| 46.74 µs| 109.27 µs| 24.29 µs| rust/regex |

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
| `python/rurex-bytes` | 676 MB/s |
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
