## Report on task: Telegram extraction speedup — flat findall and cold-str prefilter

Ad-hoc owner request: "Check telegram benchmark results. We need improve
rurex for that benchmark." The work targeted the two costs measured on
the 1M-post `@username` corpus: per-call/per-match `findall` overhead,
and the cold UCS→UTF-8 transcode that dominates one-shot `str` scans.

### Done

**`findall` allocation slimming (`src/regex.rs`).**

- Patterns without capture groups now use `Regex::find_at` in the scan
  loop instead of `captures_at`. `captures_at` allocates a `Captures`
  object on every call, including no-match calls; `find_at` allocates
  nothing.
- All matches are written into one flat `Vec<Option<(usize, usize)>>`
  instead of a `Vec` per match, then chunked into result values. An
  empty scan returns `PyList::empty` directly.
- Zero-width stepping was factored into `step_source`; the old
  `cpython_step` and the new `cpython_step_flat` share it, so the
  CPython >=3.7 empty-match retry semantics are unchanged for
  `findall`, `finditer`, `split`, `sub`, and the scanner.
- `retry_engine()` now returns immediately for patterns whose HIR has a
  positive minimum length; a non-nullable pattern never builds the
  PikeVM that it cannot need.

**Cold-str required-prefix prefilter (`src/prefilter.rs`, ADR-0013).**

- At compile time, `regex-syntax`'s prefix-literal extractor computes a
  set of literals that every match must start with. When the sequence
  is finite, non-empty, and every literal starts with an ASCII byte,
  rurex keeps up to three distinct first bytes.
- On a cold non-ASCII `str`, the raw UCS storage is scanned for one of
  those bytes before transcoding. UCS-1 is a direct `memchr`; UCS-2/4
  use `memchr` plus unit-alignment and zero-byte checks, so `0x40`
  inside an unrelated code unit is not mistaken for ASCII `@`. A
  false-hit cap falls back to "may match" on pathological input.
- Absence means no match can exist, so `findall`, `finditer`,
  `search`, `match`/`prefixmatch`, `fullmatch`, and `is_match` return
  their normal no-match answer without transcoding or scanning.
  `split`/`sub`/`subn` keep the unfiltered path for now (their
  no-match answers must preserve `str`/`bytes` copy and subclass
  behavior).
- Conservative gates: infinite literal sequences, empty literals,
  non-ASCII first bytes, NUL, more than three distinct bytes, or a
  parse failure disable the prefilter. Bytes patterns and warm UTF-8
  haystacks never consult it.

**Lone-surrogate decision.** Owner confirmed the API must follow `re`.
Skipping the transcode now returns the no-match answer for a lone
surrogate, which is what `re` does; full parity for haystacks that
still reach the transcoder is recorded in `plan.md` as a new task.

**Tests and docs.**

- 8 Rust unit tests for the prefilter: extraction cases, nullability,
  UCS-1/2/4 scanning, endian alignment false positives, a multi-byte
  candidate set, and a differential "false implies absent" check.
- `tests/test_prefilter.py` locks the user-visible contract against
  stdlib `re` for all read-only query methods, alignment false
  positives, case folding, non-ASCII prefixes, group results with
  `pos`/`endpos`, and the lone-surrogate no-match case.
- ADR-0013 added; `architecture.md`, `index.md`, README performance
  text, the generated Telegram page, and the plan task updated.
- Pre-existing `clippy::items_after_test_module` failures in
  `src/error.rs` and `src/flags.rs` (new in clippy 1.97) were fixed by
  moving the test modules to the end of the file.

### Measured

Full `benchmarks/telegram.py` run, release builds, fresh process per
point (single cold run; the table below is the committed result):

| interpreter | workers | engine | input | before | after | change |
|---|---:|---|---|---:|---:|---:|
| GIL | 1 | rurex | str | 0.754 | 0.556 | -26% |
| GIL | 1 | rurex | bytes | 0.352 | 0.290 | -18% |
| GIL | 1 | re | str | 0.468 | 0.516 | +10% |
| GIL | 1 | re | bytes | 0.510 | 0.536 | +5% |
| GIL | 4 | rurex | str | 0.947 | 0.686 | -28% |
| GIL | 4 | rurex | bytes | 0.621 | 0.449 | -28% |
| free | 1 | rurex | str | 0.887 | 0.613 | -31% |
| free | 1 | rurex | bytes | 0.417 | 0.365 | -12% |
| free | 4 | rurex | str | 0.365 | 0.302 | -17% |
| free | 4 | rurex | bytes | 0.302 | 0.290 | -4% |

Ratios in the committed result: GIL 1 thread rurex/re is ×1.1 str and
×0.5 bytes; free 1 thread is ×1.3 str and ×0.9 bytes. A 300k-post
sample showed the apparent transcode+prefilter cost fall from 0.31 µs
to 0.04 µs per post, and the bytes path from 0.437 to 0.399 µs per
post; 300k outputs were differentially checked against stdlib `re`
with zero mismatches.

The main `benchmarks/run.py` suite was also re-run. Extraction improved
(Test 3 about ×2.3, Test 4 ×1.74 at 160 matches/KB, versus the
committed page's ×1.4 and parity), but the search-bound rows shifted
with CPU state (Test 1's 16 MB point fell from ×13.4 to ×6.5). Following
the precedent of the Tier 1 report, the committed `results.json` and
main charts were kept rather than replacing the published snapshot with
a noisy run; the extraction improvement is recorded here.

### Spec/ADR amendments

- [acted] `spec/docs/adr/0013-cold-str-required-prefix-prefilter.md`
  records the extractor contract, UCS scan, gates, and rejected
  alternatives; `index.md` catalogues it.
- [acted] `spec/docs/architecture.md` now shows `src/prefilter.rs`,
  describes the flat `findall` path and nullable-only retry engine,
  and names ADR-0013 in the unsafe/safety section.
- [acted] `spec/docs/plan.md` has the new "Full lone-surrogate parity
  for str haystacks" task.
- [acted] `docs/telegram-benchmark.md`, its charts, and the README
  performance text now tell the current story (prefilter, near-parity
  on GIL str, bytes lead).

### Future-task notes

- [open] Wire the prefilter into `split`/`sub`/`subn`; their no-match
  answers need a plain `str`/`bytes` copy without forcing a transcode.
- [open] Reuse `CaptureLocations` for `findall` patterns with capture
  groups; the flat buffer removed the per-match `Vec`, but
  `captures_at` still allocates a `Captures` per matching call.
- [acted] The single cold run per benchmark point remains noisy
  (carried over from the Tier 1 report); min-of-N or recorded variance
  would make published numbers comparable run to run.
  [acted — implemented with CPU-pinned warm-up, affinity, load and
  frequency recording, and median-of-3 publication in
  report-1792358338-measurement-hardening.]
- [open] AVX-512/aarch64 NEON transcoder kernels and folding the
  transcode into the detached region for large haystacks remain
  unchanged follow-ups from ADR-0012.
- Full lone-surrogate parity is the new `plan.md` task.

### Tooling/process

- **The `.venv` build profile is a trap.** `make test-all` runs
  `maturin develop` without `--release`, so after the checks the
  installed extension is a debug build. Benchmarks then look several
  times slower. Always run `VIRTUAL_ENV=... maturin develop --release`
  before measuring; the final state here was rebuilt release for both
  `.venv` and `.venv-t`.
- **Clippy 1.97 added `items_after_test_module`.** Two files that were
  clean under earlier toolchains failed `-D warnings`; moving the test
  modules was mechanical.
- The prefilter's `memchr` dependency is already in `Cargo.lock`
  through `regex`; only a direct dependency entry was added.
- A Rust-only probe in `var/findall_probe` confirmed the allocation
  change before touching the binding: `captures_at` + `Vec<Vec>` 51.6
  ns versus `find_at` + flat `Vec` 29.9 ns on a no-match 515-byte
  haystack, and 1846 versus 1132 ns with ten matches.
