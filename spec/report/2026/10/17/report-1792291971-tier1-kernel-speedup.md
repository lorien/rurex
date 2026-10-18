## Report on task: Tier 1 speedup — transcoder kernel + per-call cost

Ad-hoc owner request: "the rurex is still slower than `re` — speed it up
more", with the explicit framing that the Telegram `@username` case is
only the measuring stick: **we are speeding up any search, not that
one pattern**. Owner decisions: do Tier 1 (unconditional, every call)
first, measure, then discuss Tier 2.

### Done

**Measurement first — where the time goes** (300k real posts, `var/`
probes):

| per post (515 B UTF-8) | |
|---|---|
| rurex `findall` bytes (engine + call) | 0.230 µs |
| rurex `findall` str | 0.791 µs |
| `re` `findall` str | 0.511 µs |
| → transcode + str delta | 0.561 µs |

Decomposing the transcode by input size showed **no fixed per-call
cost at all**: 0.92–1.00 ns/char from 50 to 31,889 units. It is 100%
kernel rate plus memory traffic on the string's own bytes (scattered
short strings cost ~1.5 ns/char, one contiguous block ~1.0). So the
engine is already 2.2× faster than `re` on the same bytes; the entire
remaining gap is the transcode.

**Landed:**

- **Length-pair staging** (`src/utf8/simd.rs`): a chunk whose lanes
  are only 1- and 2-byte output (70.7% of chunks in this corpus —
  measured) stages `lead | tail<<8` with two ops per lane instead of
  the full four-candidate blend chain; likewise 3|4-only chunks; the
  general chain only for chunks that mix across pairs (~11%).
- **One loop per UCS kind** instead of a `match kind` inside the chunk
  loop — the latter compiled to ~8 conditional branches per chunk
  (load width and the surrogate test are loop-invariant), which
  showed up as branch mispredicts on varied text.
- **Broadcast constants moved to `.rodata`** so the hot loop loads
  them instead of re-materializing per chunk.
- **Table-driven compaction kept** (the 256-row `[len, shuffle]`
  table); confirmed better than arithmetic `pext`/`pdep` control
  building on real text (0.71 vs 0.62 GB/s in the kernel harness).
- **`Utf8Scratch`** (`src/utf8.rs`): the transcode writes into a
  per-thread pooled buffer instead of a fresh `Vec` per call
  (allocator pressure, and a warm buffer between calls).

**Measured** (min-of-3/4, same protocol both sides):

| | before | after |
|---|---|---|
| kernel rate (501-unit blocks) | 1.45 ns/char | **0.98 ns/char (1.48×)** |
| per-post transcode (scattered) | 0.556 µs | 0.481 µs (hot) / 0.401 → 0.318 (cold) |
| Telegram row, GIL 1 thread | 0.76 s (×1.65 of `re`) | **0.69 s (×1.50)** |
| Telegram row (single cold run) | 0.77 s | 0.75 s |
| bytes rows / warm / ASCII | — | unchanged (control) |

**Negative results (measured and rejected — recorded in the module
doc so nobody retries them blind):**

- **Two-pass prefix-sum offsets** (classify a block, prefix-sum the
  output lengths, emit in parallel): 1.56× faster in a synthetic
  harness but **slower end to end** (0.98 → 1.58 ns/char) — the second
  classify pass costs more than the decoupling saves on this CPU.
- **Per-chunk byte count from a `popcount` of the keep-mask** instead
  of the table row: slower (the table row load was never what gated
  the store).
- **Uniform 2/3/4-byte fast paths** (fixed shuffles): 1.03 vs 0.98 —
  the branches cost more than the shuffle saves; the table holds
  those rows anyway.
- **8-byte stores for the 1|2 path**: no measurable gain.

**Structural conclusion:** the transcode's remaining cost is
partly *memory traffic on the string's own bytes* — no faster encoder
can remove it. That is why the row moves only ~9% while the kernel is
1.5× faster. Getting the str row to `re` parity (or better) requires
**not transcoding most strings**, which is Tier 2: a source-side
first-unit prefilter over the UCS buffer before the transcode
(required literal or character class derived from the HIR; absent →
the operation's no-match answer; present → transcode from the first
hit). Measured premise: **77.2% of posts contain no `@` at all**, so
three quarters of the corpus would skip the transcode entirely. This
is general machinery for any pattern with a determinable first
element (`the-needle-string`, `\d{3,4}`, `(x+x+)+y`, `https?://…`),
not an `@username` special case — and it is gated to the
cold-transcode path only, so ASCII, warm-cache and bytes inputs never
pay for it.

### Spec/ADR amendments

- [acted] `docs/telegram-benchmarks.md` prose now quotes the results
  table through interpolation (`w()`/`x()` in `telegram_charts.py`)
  instead of hand-copied numbers, so a re-run cannot leave the prose
  telling last run's story.
- [acted] `src/utf8/simd.rs` module doc describes the shipped design
  and lists the measured-rejected variants (two-pass, popcount
  lengths, uniform paths, arithmetic control).
- No ADR change needed: ADR-0012 already authorizes this `unsafe`
  fast path and its shape (kernel behind runtime detection with a
  scalar fallback).

### Future-task notes

- [acted] **Tier 2 — source-side first-unit prefilter** (the change
  that can actually reach `re` parity on the str row; see the numbers
  above). Skips the transcode for patterns whose required first unit
  is absent, transcodes from the first hit otherwise; strictly
  non-regressive by construction.
  [acted — shipped in report-1792325206-findall-prefilter-speedup via
  regex-syntax's prefix extractor and a UCS-alignment memchr scan;
  GIL 1-thread str 0.75 s → 0.56 s.]
- [open] The single cold run per benchmark point is noisy at the
  ~±10-20% level on these rows (`re` swung 0.44–0.67 s across runs).
  The A/B above uses min-of-4; consider making the benchmark itself
  take min-of-N (or record variance) so published numbers are
  comparable run to run.
- [open] AVX-512 and aarch64 NEON kernels (other architectures keep
  the scalar encoder, unchanged).
- [open] Fold the transcode into the detached-GIL region for ≥ 4 KB
  haystacks (parallel encode for big-text pipelines).

### Tooling/process

- **`perf` is installed but unusable without privileges**
  (`perf_event_paranoid = 3`). It is the right instrument for the
  remaining kernel question (why the compaction runs at ~1 uop/cycle
  instead of ~4); to enable it:
  `sudo sysctl -w kernel.perf_event_paranoid=2` (or grant
  `CAP_PERFMON`). Black-box A/B loops found the big wins but cannot
  see stalls.
- **Beware `__m256i` through plain `fn` pointers**: a `#[target_feature]`
  function called through a feature-less fn-pointer type gets the
  wrong (non-AVX) ABI and the vectors arrive as garbage. The first
  "kernel bug" hunt in the harness was this, not the kernel.
- **Reproduce measurements before believing them**: the same code was
  measured at 0.6 and 1.0 GB/s depending on run order and thermal
  state; min-of-N plus an A/B against the stashed baseline is the
  only comparison worth reporting.
- `cargo fmt`/`clippy -D warnings`/`cargo test`, pytest 279, CPython
  vendored suite (71 + 159 subtests), rustregex corpus (2,406),
  divergences (1,078), free-threaded 3.14t (279), abi3 build, and the
  main 9-test suite re-run (re/rurex ratios unchanged — the ASCII
  regime is untouched; absolute times shifted with CPU state, so the
  committed `benchmarks/results.json` snapshot was kept).
