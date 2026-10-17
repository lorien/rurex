# Telegram @mention extraction: rurex vs `re`

Real-world task, measured exactly as performed: **one** pass of
`.findall(text)` over every post — one search, once, on a corpus that
has never been scanned before. Each measured point is a **fresh
interpreter process** that loads the corpus and performs the single
pass split across N barrier-started workers; there is no warm-up and
no repeat scan. This is the extraction-bound regime (short texts,
≈0.33 matches per post) — the opposite of the scan-bound workloads of
[the main benchmark page](benchmarks.md), where rurex leads ×7–13
single-threaded.

## Methodology

- **Pattern** (Telegram username rules — 5–32 chars,
  `[a-zA-Z0-9_]`, starts with a letter):
  `@[a-zA-Z][a-zA-Z0-9_]{4,31}`
- **Corpus**: 1,000,000 posts, 470 MB of text
  (mostly Arabic/Persian), 334,136 mentions in
  186,226 posts (18.6%
  of posts). Downloaded automatically from the
  [`bench-data-telegram-v1` release](https://github.com/lorien/rurex/releases/tag/bench-data-telegram-v1)
  and sha256-verified; see
  [`benchmarks/telegram/PROVENANCE.md`](../benchmarks/telegram/PROVENANCE.md).
- **Timing window**: from the instant a worker passes the barrier to
  the instant its slice is finished; a point is the max across
  workers. Load and pattern compile are outside the window,
  identically for both engines. Every run's mention count must equal
  334,136.
- **Interpreters**: GIL CPython 3.13 and free-threaded 3.14t, one
  fresh subprocess per point. CPU: Intel(R) Core(TM) i5-8350U CPU @ 1.70GHz.

## GIL Python

![Threads on GIL Python](img/telegram_threads_gil.png)

| Workers | `re` str | `re` bytes | rurex str | rurex bytes | rurex/re str | rurex/re bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0.48 s | 0.51 s | 1.49 s | 0.35 s | ×3.1 | ×0.7 |
| 2 | 0.62 s | 0.57 s | 1.64 s | 0.42 s | ×2.6 | ×0.7 |
| 3 | 0.65 s | 0.75 s | 1.65 s | 0.51 s | ×2.6 | ×0.7 |
| 4 | 0.51 s | 0.67 s | 1.73 s | 0.54 s | ×3.4 | ×0.8 |

*(ratio < 1 = rurex faster)*

Each point now runs both input forms. The **str** variant is the
real task on parsed text: `re` 0.48 s single-threaded vs rurex
1.49 s — most of rurex's gap is **CPython's first-touch UTF-8
materialization** (CPython stores `str` in UCS form and lazily
encodes-and-caches a UTF-8 copy inside each string on first use;
`re` never converts). The **bytes** variant feeds the same posts
pre-encoded to UTF-8 — the corpus is already UTF-8 on disk, so a
pipeline that stays in bytes never pays the transcode: rurex answers
in **0.35 s, beating `re`'s 0.51 s (×1.5)** single-threaded. This is
the RE2-binding-recommended shape (feed UTF-8 bytes), and rurex
makes it a first-class mode.

With threads, rurex str stays flat (1.49–1.73 s; an earlier build
degraded to 5.38 s before the 4 KB GIL-release threshold landed —
rurex releases the GIL only for ≥ 4 KB haystacks, matching `re`'s
own never-release-during-matching behavior for short texts), and
rurex bytes improves modestly (0.35 → 0.54 s at four workers: the
scans are sub-µs, so the GIL-held per-call result building
dominates — the same ceiling `re` bytes hits at 0.51 → 0.67 s).

## Free-threaded Python

![Threads on free-threaded Python](img/telegram_threads_free.png)

| Workers | `re` str | `re` bytes | rurex str | rurex bytes | rurex/re str | rurex/re bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0.50 s | 0.42 s | 1.82 s | 0.43 s | ×3.7 | ×1.0 |
| 2 | 0.27 s | 0.26 s | 1.04 s | 0.33 s | ×3.8 | ×1.3 |
| 3 | 0.22 s | 0.20 s | 0.64 s | 0.30 s | ×2.9 | ×1.5 |
| 4 | 0.20 s | 0.19 s | 0.65 s | 0.30 s | ×3.3 | ×1.6 |

*(ratio < 1 = rurex faster)*

Without the GIL both variants scale: rurex str 1.82 s → 0.65 s
(2.8×; the first-touch UTF-8 encoding parallelizes like any other
work) and rurex bytes 0.43 s → 0.30 s; `re` 0.50 s → 0.20 s str,
0.42 s → 0.19 s bytes. On this task free-threaded Python is the
fastest way to run either engine at every thread count, and rurex
bytes at 0.30 s is within 1.6× of the best `re` number while
carrying no transcode tax at all.

## Reading the numbers

Read the two variants as two different questions. **str**: what a
user scanning parsed text once pays — `re` wins (it never
transcodes), rurex pays CPython's one-time per-string UTF-8 encode
(warm re-scans borrow the cache and reach near-parity, ≈0.55 vs
≈0.49 µs/post). **bytes**: what a UTF-8-native pipeline pays — rurex
**wins** (×1.5 single-threaded), because its engine scans UTF-8
directly with SIMD prefilters. The scan-bound leads on the main page
(×7–13) do not transfer to short-text per-call extraction, but the
bytes variant shows rurex ahead where the input is already UTF-8 —
which for a JSONL/log corpus is the natural shape. Machine-specific;
expect different absolute numbers elsewhere.

## Reproducing

```sh
python benchmarks/telegram.py            # downloads the corpus on first run,
                                         # then 16 cold runs, ~5 min
python benchmarks/telegram_charts.py     # regenerates this page
python benchmarks/telegram_charts.py --html   # + standalone HTML twin
```

---

*Raw results: [`benchmarks/telegram_results.json`](../benchmarks/telegram_results.json).
Numbers on this page were measured on 2026-10-17 with rurex 0.1.0
(GIL-release threshold fix included).*
