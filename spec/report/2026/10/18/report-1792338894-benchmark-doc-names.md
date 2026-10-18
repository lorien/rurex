## Report on task: Benchmark document naming policy

Ad-hoc owner request: standardize the generated benchmark page names.

### Done

- `docs/telegram-benchmarks.md` and `.html` renamed to
  `docs/telegram-benchmark.md` and `.html`.
- `docs/html-benchmarks.md` and `.html` renamed to
  `docs/homepage-benchmark.md` and `.html`.
- `benchmarks/telegram_charts.py` and `benchmarks/frontend_html_charts.py`
  now write the new paths, and their embedded HTML banners point at the
  new Markdown files.
- Updated `.gitignore`, README links, `docs/benchmarks.md` cross-links,
  the Telegram provenance note, and the active benchmark reports.
- Regenerated both Markdown pages and the ignored HTML twins so their
  internal links use the new names.
- Historical session reports that mentioned the old paths were updated
  to the current names to avoid dead references.

### Verification

- `rg` finds no remaining `telegram-benchmarks` or `html-benchmarks`
  references outside git history.
- Both chart generators run and write the new files.
- `make check` passes (45 Rust tests, 295 pytest tests, 3,484
  conformance tests).

### Future-task notes

- [open] Any external release notes or links outside this repository
  that point at the old benchmark page names will need the same rename.
