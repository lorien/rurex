## Report on task: Frontend HTML href/src benchmark

Ad-hoc owner request: add a second real-world benchmark using the
owner-provided frontend HTML corpus, upload that corpus to GitHub
Releases like the Telegram data, and measure rurex against `re`.

### Done

**Corpus and release.**

- Verified the cleaned `var/front.tgz`: 2,086 files, all `.html`,
  82,982,461 bytes compressed, 503,179,497 bytes decoded, all valid
  UTF-8.
- Created release `bench-data-html-v1` and uploaded the exact file as
  `frontend-html-500mb-v1.tgz`. GitHub reports the same SHA-256:
  `a38962acb0b21bb603e8c0c7f4dfa357146e7c798d4041477b52773c1f53a330`.
- `benchmarks/frontend_html.py` downloads and sha256-verifies the
  asset, caches it under `var/bench-data/`, and builds a
  length-prefixed binary corpus for fast fresh-process loads. The
  default run loads the first 256 MiB (1,127 documents); `--full` loads
  all 2,086; `--quick` loads 32 MiB.
- `benchmarks/html/PROVENANCE.md` records the release URL, all
  checksums, the archive/decoded sizes, the sampling rule, and the
  owner-provided source.

**Benchmark.**

- One dense `findall` workload only:
  `(?i)\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))`.
- `str` and raw UTF-8 `bytes`, 1–4 workers, GIL 3.13 and free-threaded
  3.14t, one fresh process per point, barrier-started single pass.
- Every point asserts the match count. A stats pass computes the match
  hash with stdlib `re` and rurex and aborts if the two disagree.
- Full-corpus reference: 598,424 matches in 2,080 documents, median
  192 per document, max 4,692. Default subset: 335,801 matches in 1,125
  of 1,127 documents; stats hash matches between engines.

**Results** (default 256 MiB subset, one cold run per point):

| interpreter | workers | `re` str | rurex str | `re` bytes | rurex bytes |
|---|---:|---:|---:|---:|---:|
| GIL | 1 | 8.77 s | 1.39 s | 4.31 s | 1.10 s |
| GIL | 4 | 7.88 s | 0.81 s | 4.37 s | 0.57 s |
| free | 1 | 7.48 s | 1.45 s | 4.66 s | 1.13 s |
| free | 4 | 2.22 s | 0.94 s | 1.58 s | 0.62 s |

rurex wins both forms at every thread count. Unlike Telegram, the
`str` case is also a clear rurex lead: documents have a median of
117 KB, so the cold transcode amortizes. The documents cross the 4 KB
GIL-release threshold, so rurex scales under the GIL while `re`'s
`str` row stays flat.

**Memory measured.**

- Default 256 MiB subset: max RSS 640 MiB for `str`, 281 MiB for
  `bytes`.
- Full 480 MiB: max RSS 1.18 GiB for `str` (measured with a direct
  `single_point` call). Full bytes was not measured.

**Docs and tooling.**

- Added `benchmarks/frontend_html.py`, `benchmarks/frontend_html_charts.py`,
  `benchmarks/frontend_html_results.json`, `docs/homepage-benchmark.md`,
  and the two charts.
- Added `make bench-html`; updated `spec/docs/dev.md`, `docs/benchmarks.md`,
  README, and `.gitignore` for the generated HTML twin.
- The release upload used the token from `.env`; no credential was
  printed or committed.

### Spec/ADR amendments

- [acted] `docs/benchmarks.md` now links both real-world cases:
  Telegram and frontend HTML.
- [acted] `spec/docs/dev.md` documents `make bench-html`.
- No ADR: the library is unchanged; this is benchmark infrastructure.

### Future-task notes

- [open] Decide whether the default 256 MiB subset is the right RAM
  trade-off. Measured `str` RSS is 640 MiB (not the ~300 MB estimated
  before the run); a 128 MiB default would roughly halve it.
- [open] `make bench-html` regenerates the committed results and docs,
  same as the other benchmark targets; a scratch-then-promote flow
  would avoid accidental snapshot replacement.
- [open] A sparse `<title>` or literal-scan pattern was deliberately
  left out at the owner's request; the dense attribute pattern already
  exposes the long-haystack engine lead.

### Tooling/process

- **Never name a benchmark script `html.py`.** It shadows the stdlib
  `html` package because `sys.path[0]` is `benchmarks/`; importing
  matplotlib then fails inside pyparsing's `import html.entities`.
  The files are `frontend_html.py` and `frontend_html_charts.py`.
- The corpus archive contains only `.html` members; the builder fails
  loudly on any other file, so a future dirty archive cannot silently
  change the corpus.
- The binary cache is 480 MiB on disk; the benchmark reads only the
  requested subset, so RAM use is independent of the cache size.
