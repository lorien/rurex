# Vendored corpus: regex-engine-divergences

Source: <https://github.com/Kristofp/regex-engine-divergences>,
vendored partially (see below).

- Repository: Kristofp/regex-engine-divergences
- Commit: `668aeffaa0e0dac1e977ee00eccc14a9caa5bb24` (branch `main`;
  the repo is a published snapshot without tags)
- Vendored: 2026-10-17
- License: MIT (`LICENSE-MIT` here)
- Contents: `constructs.json` — 539 curated regex constructs (pattern,
  testText, flags, category) chosen where real engines disagree;
  `ground-truth/python.json` and `ground-truth/rust.json` — answers
  recorded by the upstream authors on *their* runtimes.

**These tests are inputs, not oracles** (ADR-0011). The comparison
targets are live: rurex vs stdlib `re` on this interpreter (mode L),
and rurex vs the engine contract — the native `regex` crate behavior
plus rurex's documented adjustments (mode E, using the recorded Rust
answers as a discussion reference, never an auto-fail). Recorded
answers are kept for triage context; drift between them and the live
engines is upstream-version noise, not a rurex failure.

The recorded operation (from upstream `runners/run_python.py`):
compile with `i`/`m`/`s`/`x` flag mapping (other flag letters,
including `g`, ignored), then `finditer` over the whole test text,
reporting `(match text, start index)` for every match plus a
compile-verdict error flag.

Re-download with:

```sh
curl -sL https://github.com/Kristofp/regex-engine-divergences/archive/668aeffaa0e0dac1e977ee00eccc14a9caa5bb24.tar.gz \
  | tar xz --strip-components=1 -C tests/divergences/data \
  regex-engine-divergences-main \
  --wildcards 'constructs.json' 'ground-truth/python.json' 'ground-truth/rust.json' LICENSE
```
