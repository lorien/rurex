## Report on task: Telegram @mention real-world benchmark

Ad-hoc owner request: a new committed document
`docs/telegram-benchmark.html` benchmarking @username extraction
over 1M Telegram channel posts, thread series 1–4, GIL and
free-threaded Pythons.

### Done

- Data: `tg-1m.json.gz` (93 MB gzip of the 1,000,000-record JSONL).
  Initially committed to git (527 MB raw) by owner decision, then
  removed again by owner decision the same day: the two unpushed
  commits carrying it were dropped via `git reset --hard` (the remote
  never received them, so no force-push was needed). The corpus now
  lives as a GitHub release asset, downloaded once into the
  gitignored `var/bench-data/` cache and sha256-verified by the
  harness; see `benchmarks/telegram/PROVENANCE.md`.
- Harness `benchmarks/telegram.py`: untimed JSONL load (~6 s), corpus
  stats pass, barrier-started pile protocol (1M posts split across
  1–4 workers; op = one `findall` per post; wall = max per-thread;
  mention-count checksum per pile), GIL run in-process +
  free-threaded run via `.venv-t` subprocess, `--quick`/`--data`
  CLI, results JSON. Chart generator `benchmarks/telegram_charts.py`
  → committed standalone HTML with embedded charts.
- Pattern: `@[a-zA-Z][a-zA-Z0-9_]{4,31}` (Telegram username rules).
- Corpus: 334,136 mentions in 186,226 posts (18.6% of posts, ≈0.33
  mentions/post), 290 MB of mostly multibyte text.

### Findings (first run — SUPERSEDED, kept for the record)

The first protocol measured each point once in one shared process
with no warm-up, mixing one-time costs into some points but not
others. Its three findings are retracted:

- ~~re wins 2.8× single-threaded~~ — inflated: rurex's first measured
  pile absorbed CPython's first-touch UTF-8 materialization for the
  whole corpus.
- ~~both engines parallelize on GIL Python~~ — true only for the
  warmed state; the cold behavior is different (below).
- ~~rurex degrades on free-threaded (1.60 → 3.93 s)~~ — a
  measurement artifact of the mixed protocol plus single-measurement
  noise; it does not reproduce.

### Corrected protocol and findings (owner-directed)

One real-world run per point: a fresh interpreter process loads the
corpus and performs the single pass — one search, once, no warm-up,
no repeats. 16 cold runs (2 interpreters × 4 thread counts × 2
engines), mention checksum 334,136 per run.

- Root cause of the first run's anomalies, proven by chunked timing
  and a ctypes pre-warm of the UTF-8 cache: **CPython's first-touch
  UTF-8 materialization**. `PyUnicode_AsUTF8AndSize` (which PyO3's
  `&str` extraction — and thus rurex — calls per haystack) encodes
  and caches a UTF-8 copy inside each str the first time; `re` never
  converts. Over 470 MB of mostly non-ASCII text this is a ~1 s
  one-time cost, paid inside the measured run because a scan-once
  user pays it.
- **GIL Python:** `re` 0.48 s (1t), flat-ish across threads; rurex
  1.65 s (1t) degrading to 5.38 s (4t) — the UTF-8 allocation runs
  in rurex's GIL-held section and the pile performs a million
  detach/attach cycles: a GIL convoy that grows with thread count.
- **Free-threaded:** the same cold scan scales cleanly — rurex
  1.46 → 0.52 s (2.8×), `re` 0.42 → 0.33 s. Free-threaded Python is
  the fastest way to run this task with rurex at every thread count.
- Warm steady state (for reference, separate probes): both engines
  scale on both builds; rurex ≈0.55 µs/post vs `re` ≈0.49 warm —
  near-parity single-thread.

### Spec/ADR amendments

- [acted] docs/benchmarks.md cross-link; new committed
  docs/telegram-benchmark.html.

### Future-task notes

- [open] Investigate rurex free-threaded findall degradation
  (1.6 → 3.9 s): profile allocation/refcount contention vs PyO3
  free-threaded object paths; candidate mitigations — batched
  result building, per-thread caches.
  [acted — resolved as a measurement artifact: the mixed
  warm/cold protocol plus single-measurement noise; see the
  corrected findings above. Nothing to fix in rurex.]
- [open] Re-measure when PyO3/3.14t land free-threading
  improvements; single-measurement points are noisy in the free-t
  `re` series (0.51–0.85 s non-monotonic).
  [acted — moot: the fresh-process-per-point protocol replaces it;
  free-t `re` is now 0.42/0.27/0.22/0.33 s.]
- [open] GIL-build cold-scan convoy (rurex 1.65 → 5.38 s with
  threads): candidate mitigations if scan-once-under-threads on GIL
  Python matters to users — move the haystack extraction (UTF-8
  materialization) inside the detached region, or batch per-thread.
  [acted — fixed in report-1792259247-gil-release-threshold: release
  the GIL only for ≥4 KB haystacks; GIL cold scan now flat at
  1.55–1.88 s across 1–4 workers.]

### Tooling/process

- The corpus release asset is live at
  `bench-data-telegram-v1`; the harness download+sha256 chain was
  validated end-to-end (cache deleted -> download -> verify -> run).
- `gh`'s keyring token cannot write (HTTP 403 on releases); the
  write-capable `GITHUB_TOKEN` lives in `.env` and must be sourced
  first — now documented in AGENTS.md.
- A GIL-hog control thread is the cheap decisive test for "does X
  release the GIL": compare the pile wall with and without a
  spinning pure-Python thread — settled the _sre question without
  reading CPython sources.
- 100k-post `--quick` piles are too short for stable thread numbers
  (±50% noise); full 1M piles are stable to a few percent.
