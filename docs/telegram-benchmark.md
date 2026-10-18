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
| 1 | 0.52 s | 0.54 s | 0.56 s | 0.29 s | ×1.1 | ×0.5 |
| 2 | 0.52 s | 0.63 s | 0.60 s | 0.39 s | ×1.1 | ×0.6 |
| 3 | 0.61 s | 0.72 s | 0.77 s | 0.46 s | ×1.3 | ×0.6 |
| 4 | 0.63 s | 0.60 s | 0.69 s | 0.45 s | ×1.1 | ×0.7 |

*(ratio < 1 = rurex faster)*

Each point runs both input forms. The **str** variant is the real
task on parsed text: `re` matches the UCS buffer directly, while
rurex's engine matches UTF-8 and must materialize it. The remaining
gap is now small (×1.1 single-threaded): the
required-first-byte prefilter (ADR-0013) skips the transcode for the
77% of posts that cannot contain a match, so only posts with a
candidate `@` pay it, and a hand-written AVX2 kernel (ADR-0012)
handles those at 1–4 GB/s. Strings whose CPython UTF-8 cache another
C-API consumer already filled are borrowed zero-copy. The `findall`
path itself was also slimmed: it uses `find_at` and one flat span
buffer instead of a `Captures` object and a `Vec` per match.

The **bytes** variant feeds the same posts pre-encoded to UTF-8 —
the corpus is already UTF-8 on disk, so a pipeline that stays in
bytes never pays the transcode: rurex answers in
**0.29 s, beating `re`'s
0.54 s (×0.5)**
single-threaded. That is the RE2-binding-recommended shape (feed
UTF-8 bytes), and rurex makes it a first-class mode.

With threads, both rurex variants get slower under the GIL
(str 0.56 → 0.69 s,
bytes 0.29 → 0.45 s
at four workers): the scans are sub-µs, so the GIL-held per-call result
building dominates and the workers convoy on the lock. `re` bytes
shows the same shape
(0.54 → 0.60 s). An earlier
build degraded to 5.38 s before the 4 KB GIL-release threshold landed;
releasing the GIL for every tiny scan was worse.

## Free-threaded Python

![Threads on free-threaded Python](img/telegram_threads_free.png)

| Workers | `re` str | `re` bytes | rurex str | rurex bytes | rurex/re str | rurex/re bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0.48 s | 0.41 s | 0.61 s | 0.37 s | ×1.3 | ×0.9 |
| 2 | 0.28 s | 0.26 s | 0.39 s | 0.30 s | ×1.4 | ×1.2 |
| 3 | 0.22 s | 0.24 s | 0.33 s | 0.29 s | ×1.5 | ×1.2 |
| 4 | 0.21 s | 0.21 s | 0.30 s | 0.29 s | ×1.5 | ×1.4 |

*(ratio < 1 = rurex faster)*

Without the GIL both variants scale: rurex str
0.61 s → 0.30 s
(2.0×; the prefilter,
the remaining transcode and the result building all run in parallel) and
rurex bytes
0.37 s → 0.29 s;
`re` 0.48 s → 0.21 s str,
0.41 s → 0.21 s bytes.
With more than one worker free-threaded Python is the fastest way to
run either engine on this task; at one worker the GIL build is
slightly faster for rurex because per-call overhead, not the scan,
dominates. rurex bytes at 0.29 s is within
1.4× of the best `re` number while carrying
no transcode tax at all.

## Reading the numbers

Read the two variants as two different questions. **str**: what a
user scanning parsed text once pays — `re` still has a small edge on
free-threaded Python, while on the GIL build rurex is within
×1.1 at one worker because the prefilter avoids most
transcodes and `findall` no longer allocates per match. **bytes**:
what a UTF-8-native pipeline pays — rurex **wins**
(×0.5 single-threaded), because its engine
scans UTF-8 directly with SIMD prefilters. The scan-bound leads on
the main page (×7–13) do not transfer to short-text per-call
extraction, but the bytes variant shows rurex ahead where the input
is already UTF-8 — which for a JSONL/log corpus is the natural shape.
Machine-specific;
expect different absolute numbers elsewhere.

## Reproducing

```sh
python benchmarks/telegram.py          # downloads the corpus on first run,
                                       # then 16 cold runs, ~5 min
python benchmarks/telegram_charts.py   # regenerates the charts, this page
                                       # and the standalone HTML twin
```

---

*Raw results: [`benchmarks/telegram_results.json`](../benchmarks/telegram_results.json).
Numbers on this page were measured on 2026-10-18 with rurex 0.1.0;
see the raw results file for the exact environment.*
