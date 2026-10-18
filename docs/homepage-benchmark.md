# Frontend HTML href/src extraction: rurex vs `re`

Real-world task: **one** pass of `.findall(document)` over real frontend
HTML pages. Each measured point is a fresh interpreter process that
loads the corpus and performs the single pass split across N
barrier-started workers; there is no warm-up and no repeat scan.

This is the long-haystack complement to the
[Telegram benchmark](telegram-benchmark.md): documents are large
(median 117 KB), matches are dense
(191 per document, up to 4,692),
and capture groups are part of the result.

## Methodology

- **Pattern** (quoted and unquoted HTML attribute values):
  `(?i)\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))`
- **Corpus**: the owner-provided frontend HTML sample, 2,086 documents
  and 479.9 MiB decoded. The benchmark downloads
  `frontend-html-500mb-v1.tgz` from the
  [`bench-data-html-v1` release](https://github.com/lorien/rurex/releases/tag/bench-data-html-v1),
  sha256-verifies it, and caches a length-prefixed local copy. See
  [`benchmarks/html/PROVENANCE.md`](../benchmarks/html/PROVENANCE.md).
  The default run uses the first 269 MiB of the corpus
  (1,127 documents); `--full` loads all of it.
- **Timing window**: from the instant a worker passes the barrier to
  the instant its slice is finished; a point is the max across workers.
  Load and pattern compile are outside the window, identically for both
  engines. Every run's match count must equal 335,801.
- **Interpreters**: GIL CPython 3.13 and free-threaded 3.14t, one fresh
  subprocess per point. CPU: Intel(R) Core(TM) i5-8350U CPU @ 1.70GHz.

## GIL Python

![Threads on GIL Python](img/html_threads_gil.png)

| Workers | `re` str | `re` bytes | rurex str | rurex bytes | rurex/re str | rurex/re bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 8.26 s | 4.41 s | 1.48 s | 1.14 s | ×0.18 | ×0.26 |
| 2 | 8.05 s | 4.19 s | 0.87 s | 0.74 s | ×0.11 | ×0.18 |
| 3 | 7.97 s | 4.21 s | 0.75 s | 0.61 s | ×0.09 | ×0.14 |
| 4 | 8.01 s | 4.24 s | 0.74 s | 0.59 s | ×0.09 | ×0.14 |

*(ratio < 1 = rurex faster)*

Single-threaded, rurex answers the parsed-`str` form in
1.48 s against `re`'s
8.26 s (×0.18), and the
bytes form in 1.14 s against
4.41 s (×0.26).

The `str` result is the opposite of the short-post Telegram regime.
Documents here have a median of 117 KB, so the cold
UCS-to-UTF-8 transcode is amortized over far more text; the
required-first-byte prefilter still skips pages that cannot contain
`href` or `src`, and the AVX2 kernel handles the rest. The engine and
its SIMD prefilters then do the dominant work.

Threads reinforce the difference. These documents are above the 4 KB
GIL-release threshold, so rurex releases the GIL during the scan, and
source buffers of at least 32 KB also release it around the cold
transcode itself, so one worker's encode does not block the others:
`str` drops from 1.48 to
0.74 s and bytes from
1.14 to
0.59 s at four workers. `re` stays flat
on `str` (8.26 to
8.01 s) because it never releases the GIL
during matching.

## Free-threaded Python

![Threads on free-threaded Python](img/html_threads_free.png)

| Workers | `re` str | `re` bytes | rurex str | rurex bytes | rurex/re str | rurex/re bytes |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 7.67 s | 4.81 s | 1.50 s | 1.31 s | ×0.20 | ×0.27 |
| 2 | 4.75 s | 2.91 s | 1.00 s | 0.78 s | ×0.21 | ×0.27 |
| 3 | 2.92 s | 1.73 s | 0.69 s | 0.59 s | ×0.24 | ×0.34 |
| 4 | 2.12 s | 1.36 s | 0.66 s | 0.56 s | ×0.31 | ×0.41 |

*(ratio < 1 = rurex faster)*

Without the GIL both engines scale. rurex goes from
1.50 to
0.66 s on `str` and from
1.31 to
0.56 s on bytes; `re` goes from
7.67 to 2.12 s
on `str` and from 4.81 to
1.36 s on bytes. rurex keeps the lead at
every thread count. The `str` and bytes rows are close for rurex
because the transcode is a small part of a long-document pass.

## Reading the numbers

This page and the [Telegram page](telegram-benchmark.md) describe the
two ends of the real-world spectrum. Telegram is short non-ASCII posts
scanned once, where per-call transcode and per-match result building
dominate; rurex had to close that gap with a prefilter and a leaner
`findall`. Frontend HTML is long UTF-8 documents with dense, grouped
extraction, where the engine and its prefilters dominate; rurex leads
on both input forms and scales under the GIL once the documents cross
the release threshold.

Machine-specific; expect different absolute numbers elsewhere.

## Reproducing

```sh
python benchmarks/frontend_html.py            # default 256 MiB subset
python benchmarks/frontend_html.py --full     # all ~480 MiB
python benchmarks/frontend_html.py --quick    # 32 MiB smoke run
python benchmarks/frontend_html_charts.py     # charts, this page, and the HTML twin
```

---

*Raw results: [`benchmarks/frontend_html_results.json`](../benchmarks/frontend_html_results.json).
Numbers on this page were measured on 2026-10-18 with rurex 0.1.0;
see the raw results file for the exact environment.*
