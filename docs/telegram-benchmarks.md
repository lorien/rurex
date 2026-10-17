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

| Workers | `re` | `re`, per post | rurex | rurex, per post | Speedup |
|---:|---:|---:|---:|---:|---:|
| 1 | 0.48 s | 0.48 µs | 1.58 s | 1.58 µs | `re` ×3.3 |
| 2 | 0.61 s | 0.61 µs | 1.55 s | 1.55 µs | `re` ×2.5 |
| 3 | 0.66 s | 0.66 µs | 1.88 s | 1.88 µs | `re` ×2.9 |
| 4 | 0.57 s | 0.57 µs | 1.63 s | 1.63 µs | `re` ×2.9 |

Single-threaded, `re` answers the pile in 0.48 s and rurex in
1.58 s. Most of rurex's gap is **CPython's first-touch UTF-8
materialization**: CPython stores `str` in UCS form and lazily
encodes-and-caches a UTF-8 copy inside each string the first time
anything asks for it (`PyUnicode_AsUTF8AndSize`) — which rurex, like
any UTF-8 engine consuming Python strings, must. `re` never converts.
Over 470 MB of mostly non-ASCII text that is a one-time ~1 s encode,
paid inside the measured run because a scan-once user pays it.

Adding threads, rurex is now **flat** (1.55–1.88 s at 1–4 workers).
An earlier build degraded badly here (1.65 s → 5.38 s): every tiny
scan released and re-acquired the GIL, and a million handoffs
convoyed the lock. rurex now releases the GIL only when the haystack
is ≥ 4 KB (see `GIL_RELEASE_MIN_HAY`); short-text scans stay
GIL-held, matching `re`'s own behavior (sre never releases the GIL
during matching at any size).

## Free-threaded Python

![Threads on free-threaded Python](img/telegram_threads_free.png)

| Workers | `re` | `re`, per post | rurex | rurex, per post | Speedup |
|---:|---:|---:|---:|---:|---:|
| 1 | 0.44 s | 0.44 µs | 1.76 s | 1.76 µs | `re` ×4.0 |
| 2 | 0.27 s | 0.27 µs | 0.83 s | 0.83 µs | `re` ×3.1 |
| 3 | 0.22 s | 0.22 µs | 0.66 s | 0.66 µs | `re` ×3.0 |
| 4 | 0.20 s | 0.20 µs | 0.53 s | 0.53 µs | `re` ×2.7 |

Without the GIL the same cold scan scales: rurex 1.76 s → 0.53 s at
four workers (3.3×) — the first-touch UTF-8 encoding parallelizes
like any other work — and `re` 0.44 s → 0.20 s. On this scan-once
task, free-threaded Python is the fastest way to run rurex at every
thread count.

## Reading the numbers

`re` wins every point on this workload: partly per-call fixed cost,
and partly the one-time UTF-8 tax that only the UTF-8 engine pays.
Users who scan each text exactly once pay it once; users re-scanning
re-use the cached UTF-8 form and the tax vanishes (steady-state
single-thread is near-parity, ≈0.55 vs ≈0.49 µs/post). The
scan-bound leads on the main page do not transfer to short-text
per-call extraction. Machine-specific; expect different absolute
numbers elsewhere.

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
