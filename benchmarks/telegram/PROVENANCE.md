# Data: 1M Telegram channel posts

- **Not in git.** The corpus is 93 MB gzip / 527 MB raw and lives as a
  GitHub release asset:
  <https://github.com/lorien/rurex/releases/tag/bench-data-telegram-v1>
  (`tg-1m.json.gz`).
- Schema (uncompressed JSONL): `{"channel_id": int, "msg_id": int,
  "text": str}` — 1,000,000 records, 526,880,801 bytes.
- Source: owner-provided Telegram research sample
  (`/web/tg-research/var/langid/sample1m.jsonl`), 2026-10-17.

Checksums (both verified by `benchmarks/telegram.py` where applicable):

| Form | SHA-256 |
|---|---|
| `tg-1m.json.gz` (93,506,895 bytes) | `eb91db735fcf1a9e6096a59e959471cb9a3c4f41438f864e800cdb66f994023b` |
| uncompressed (526,880,801 bytes) | `02c51223e24db5b05acedd88ca6123167addeebc9328e450f7d0afd4bdec6c3d` |

`benchmarks/telegram.py` resolves the corpus automatically:
`$RUREX_TG_DATA` (any `.jsonl` or `.gz`) → cached copy at
`var/bench-data/tg-1m.json.gz` → download from the release URL
(sha256-checked, downloaded once). Manual download:

```sh
curl -L -o var/bench-data/tg-1m.json.gz \
  https://github.com/lorien/rurex/releases/download/bench-data-telegram-v1/tg-1m.json.gz
```

The benchmark reads the gzip stream directly; no 527 MB file is ever
materialized. Corpus statistics (post count, bytes, mention density)
are computed by the harness at run time and reported in
`docs/telegram-benchmark.html`.
