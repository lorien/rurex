## Report on task: SIMD UCS→UTF-8 transcoder for the cold-str path

Ad-hoc owner request (follow-up from
`report-1792259247-gil-release-threshold.md`): make the cold-str
transcode fast enough to close the ×3 gap against `re` on the Telegram
workload. Owner decisions taken up front: write the SIMD kernel
ourselves rather than take a dependency; x86 first, other
architectures later only if needed; do not fold the transcode into the
detached scan region for this change.

### Done

- **Dependency survey first, and it eliminated every off-the-shelf
  option.** `encoding_rs`'s `simd-accel` is gated on nightly-only
  `core::simd` and stays scalar on stable Rust (portable_simd is still
  unstable in 1.97/1.99); `xutf` needs nightly *and* is permissive
  (lone surrogates pass through instead of raising); `simdutf` is C++
  and would put a C++ toolchain plus a dynamic libstdc++ into every
  wheel of ADR-0005's matrix. Recorded in ADR-0012.
- **Hand-written AVX2+BMI2 kernel** (`src/utf8/simd.rs`, ~330 lines):
  one kernel for UCS-1/2/4 — widen 8 units to `u32` lanes, derive the
  per-lane UTF-8 length as a 3-bit sum into a 4-bit field, emit with a
  fixed shuffle when all 8 lanes share a length, otherwise stage the
  four candidate bytes per lane and compact each 4-lane half with a
  `pshufb` controlled by a 256-row table indexed by that half's eight
  length bits. Runtime `avx2 + bmi2` dispatch cached in a `OnceLock`;
  scalar fallback everywhere else. No new dependency.

  Kernel throughput (GB/s of UTF-8 out, i5-8350U), scalar ≈ 0.48
  across the row:

  | shape | scalar | AVX2 | ratio |
  |---|---:|---:|---:|
  | arabic mixed 1\|2 (spaces) | 0.48 | 1.93 | 4.0× |
  | arabic pure 2-byte | 0.48 | 2.50 | 5.2× |
  | cjk pure 3-byte | 0.48 | 3.81 | 7.9× |
  | cjk mixed 3\|1 | 0.48 | 2.91 | 6.1× |
  | latin1 mixed 1\|2 | 0.48 | 1.44 | 3.0× |
  | latin1 pure 2-byte | 0.48 | 2.59 | 5.4× |
  | emoji mixed 1\|4 | 0.48 | 1.56 | 3.3× |
  | emoji pure 4-byte | 0.48 | 3.82 | 8.0× |
- **No surrogate-pair decoding, by design**: CPython picks the storage
  kind from the string's maximum code point, so a compact UCS-2 string
  can never hold a valid pair — every surrogate unit there is a lone
  surrogate and an error. The kernel only detects surrogates /
  out-of-range lanes in bulk and routes the whole 8-unit chunk to the
  scalar encoder, which raises at the first bad unit.
- **Fixed a latent correctness bug found while measuring:** the
  PEP 393 offsets in `src/utf8.rs` were hard-coded, but `PyObject` is
  32 bytes on free-threaded builds (not 16) and `PyCompactUnicodeObject`
  had an extra `wstr_length` before 3.12, and `Py_GIL_DISABLED` widens
  `state.interned` from 2 bits to a byte (shifting `kind`/`compact`/
  `ascii` up by 6). On 3.14t the old code read the "compact" bit out of
  a refcount word: it degraded to the slow path, and on a differently
  padded build it could have returned a bogus slice. The layout is now
  **detected** (locate `length` via two long ASCII probes, derive the
  rest from the fixed field order) and **validated** against
  `PyUnicode_AsUTF8AndSize`'s own behaviour (state bits, ASCII data,
  a warmed UTF-8 cache, UCS-2 units), cached per process; on failure
  every `str` haystack takes `to_cow`, i.e. exactly the old
  behaviour. Verified on 3.13 (GIL) and 3.14t.
- **Correctness walls:** 37 Rust tests, including the kernel-vs-scalar
  differential fuzz (9,000 cases over all three kinds, surrogates and
  out-of-range values included), boundary lengths 0–40 across seven
  alphabets, every uniform shape at chunk-aligned lengths, a
  well-formedness check on the compaction table, byte-equality with
  CPython's encoder over a torture corpus, a 4,000-case random
  differential fuzz over raw code points, lone-surrogate-at-every-
  position, and a layout-detection guard. Then pytest 279, the CPython
  vendored suite (71 passed / 159 subtests), the rustregex corpus
  (2,406) and the divergences corpus (1,078) — all green, on both the
  GIL and the free-threaded interpreter.
- **Measured:** kernel 1.44–3.82 GB/s vs 0.48 GB/s scalar (3–8×);
  Telegram cold str GIL 1-thread 1.46 s → 0.77 s (**×3.0 → ×1.6**,
  target was ×1.4–1.8), 4 threads 1.64 → 0.91 s; free-threaded
  1.47 → 0.84 s and 0.59 → 0.37 s; the bytes path is unchanged
  (0.36 → 0.35 s), which is the control that the win is the transcode.
- Docs: ADR-0012, a "Text transcoding" section plus a rewritten
  "Memory and safety" section in `architecture.md`, `index.md` entry,
  regenerated `docs/telegram-benchmarks.md` (tables *and* the prose,
  which still told the ×3 story), refreshed charts, and
  `var/utf8_micro.md` with the kernel table and the four performance
  lessons.

### Spec/ADR amendments

- [acted] `spec/docs/adr/0012-simd-transcoder-and-detected-layout.md`
  records the kernel design, the rejected dependencies, and the
  runtime-detected layout (required by `architecture.md`'s rule that
  any `unsafe` needs an ADR).
- [acted] `spec/docs/architecture.md` gains a "Text transcoding"
  section and the "Memory and safety" section now names the fast path
  as the one place `unsafe` lives, and states that PEP 393 offsets
  must be detected rather than hard-coded.

### Future-task notes

- [open] AVX-512 and aarch64 NEON kernels: the kernel is x86_64
  AVX2+BMI2 only; other architectures keep the scalar encoder
  (~0.48 GB/s). Same table design ports to NEON; AVX-512 would mainly
  help the general compaction path.
- [open] For ≥ 4 KB haystacks the transcode still runs GIL-held before
  the detached scan. Folding it into the detached closure would let
  large-text pipelines encode in parallel; deferred deliberately
  (owner decision) because the SIMD kernel already shrank the
  GIL-held burst 3–8×.
- [open] `docs/telegram-benchmarks.md` prose is hand-maintained inside
  `benchmarks/telegram_charts.py` while only the tables are computed,
  so a re-run silently leaves stale narrative. Either generate the
  numbers-in-prose too, or keep the prose purely qualitative.

### Tooling/process

- **`.venv-t/bin/maturin develop` builds for the wrong interpreter.**
  Without `VIRTUAL_ENV` (or an interpreter flag) it picked the 3.13
  interpreter, printed "🛠 Installed" while installing a cp313 wheel
  into a venv with no `pip`, and left yesterday's `cpython-314t` `.so`
  in place. Two rounds of free-threaded measurements were therefore
  taken against a stale binary and looked like "the free-threaded
  build gains nothing". Use
  `maturin build --release --interpreter .venv-t/bin/python -o
  var/wheels-t` and unzip the wheel, or export `VIRTUAL_ENV`. Also:
  `ls --time-style=+%H:%M:%S` hides the date — a "02:55" artifact can
  be from yesterday.
- **Measure the control.** The `bytes` variant of the same benchmark
  (identical engine path, no transcode) was what proved a "no change"
  free-threaded result was really "no transcode happening", and the
  `PyUnicode_AsUTF8AndSize`-cache-warming probe is what proved the
  fallback was being taken.
- Byte-order/movemask hand-reasoning cost more time than measuring:
  three separate "bugs" were misreadings of a correct 32-bit mask.
  Print the vector and the mask and compare against a computed
  expectation instead of decoding binary literals by eye.
- Keep an experimental kernel outside the extension (a pyo3-free copy
  of the kernel plus a harness under the gitignored `var/`) until it
  wins; iterating through the wheel build is minutes per round instead
  of seconds, and it makes variant-vs-variant numbers cheap. The
  numbers in this report came from `var/utf8_micro.md`, which is
  scratch and therefore not committed — the kernel table is reproduced
  in the report itself.

### Postscript: the stale HTML twin

Owner report after the commit: "I see old picture." The committed
artifacts were correct — both PNGs and `docs/telegram-benchmarks.md`
were regenerated from the final run (verified by reading the charts:
rurex/str 0.77 → 0.91 s and 0.84 → 0.37 s, the new numbers). The stale
artifact was the **gitignored HTML twin**
(`docs/telegram-benchmarks.html`): it had been written by an earlier
run, still carried the ×3 numbers, and — because it embeds the charts
as base64 — can never pick up a new PNG by itself. It was generated
only on an explicit `--html` flag, which the regeneration I ran did not
pass.

Fixed at the root rather than by re-running once:

- `benchmarks/telegram_charts.py` and `benchmarks/make_charts.py` now
  write the HTML twin **by default**; `--no-html` opts out. Both
  docstrings say why the default flipped (a self-contained snapshot
  that is opt-in silently goes stale).
- Missing `markdown` no longer skips the twin quietly: both scripts
  print a warning to stderr naming the stale file and the fix
  (`pip install markdown`).
- The HTML banner and the reproduction blocks in
  `docs/telegram-benchmarks.md` (generated) and `docs/benchmarks.md`
  (hand-written) no longer tell you to pass `--html`. The Telegram
  page's footer also stopped enumerating which fix a run "includes" —
  that sentence goes stale on every run; it points at the raw results
  file for the environment instead.
- Both twins regenerated and verified: the base64 payloads in
  `docs/telegram-benchmarks.html` now hash-equal the current PNGs.

Markers:

- [acted] HTML twin regeneration is no longer opt-in, in both chart
  scripts — a re-run cannot leave a browser-visible page on an old
  run's numbers.
