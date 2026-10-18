# Data: 2,086 frontend HTML documents

- **Not in git.** The corpus is 80 MB gzip / 479.9 MiB raw and lives as
  a GitHub release asset:
  <https://github.com/lorien/rurex/releases/tag/bench-data-html-v1>
  (`frontend-html-500mb-v1.tgz`).
- **Contents**: 2,086 files, all `.html`, stored at the archive root.
  The archive contains no index, no zip, and no non-HTML member.
- **Source**: owner-provided frontend HTML sample (`var/front.tgz`),
  fetched 2026-10-18 and cleaned to HTML-only before the release
  upload.

Checksums:

| Form | SHA-256 |
|---|---|
| `frontend-html-500mb-v1.tgz` (82,982,461 bytes) | `a38962acb0b21bb603e8c0c7f4dfa357146e7c798d4041477b52773c1f53a330` |
| uncompressed tar stream | `9e7f016194a8e8f9e883d828ae40fcc86e3f13612b2e033b2a320976a316a9ad` |
| concatenated HTML content, archive order (503,179,497 bytes) | `4a94e2bf3e08a8f0352c62c0f4fbb1268b6246edeb1fb13ecdaa87b5e0583b74` |
| local length-prefixed cache built by `frontend_html.py` | `29b0aaf018b4ad6b2ed7f88d88f392453fc188d945b38ca73c70f05917a8668b` |

`benchmarks/frontend_html.py` resolves the corpus automatically:
`$RUREX_HTML_DATA` (tar.gz) -> cached copy at
`var/bench-data/frontend-html-500mb-v1.tgz` -> download from the
release URL (sha256-checked, downloaded once). The first run converts
the archive into `var/bench-data/frontend-html-500mb-v1.bin` with a
length-prefixed record per document, so each fresh benchmark process
loads only the requested form and subset. The default run uses the
first 256 MiB (1,127 documents); `--full` uses all 2,086.

Manual download:

```sh
curl -L -o var/bench-data/frontend-html-500mb-v1.tgz \
  https://github.com/lorien/rurex/releases/download/bench-data-html-v1/frontend-html-500mb-v1.tgz
```

Corpus statistics over the full archive (stdlib `re`, pattern in
`frontend_html.py`): 2,086 documents, 503,179,497 UTF-8 bytes,
598,424 matches in 2,080 documents, median 192 matches per document,
maximum 4,692.

The HTML is published only as this benchmark asset at the owner's
request. It is not committed to the repository; users are responsible
for the terms of the original sites and for any reuse of the content.
