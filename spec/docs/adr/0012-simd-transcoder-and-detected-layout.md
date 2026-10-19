# SIMD UCS→UTF-8 transcoder and detected PEP 393 layout

Date: 2026-10-17
Status: accepted

## Context

The engine matches UTF-8, but CPython stores `str` as UCS-1/2/4.
`PyUnicode_AsUTF8AndSize` materializes a UTF-8 copy inside the string
object on first use at roughly 0.45 GB/s, and `re` never converts at
all — which is why scanning freshly-touched non-ASCII `str` cost
much more than `re` (Telegram benchmark: ×3.0 single-threaded GIL,
1.46 s vs 0.48 s for 1M posts).

ADR-0009-era work introduced a three-way borrow in `src/utf8.rs`
(borrow compact-ASCII data, borrow CPython's UTF-8 cache when warm,
transcode in Rust otherwise) and established that the scalar Rust
transcoder did not help: `String::from_utf16` runs at 0.48 GB/s, the
same class as CPython's encoder. Measurement from that session
(report `report-1792259247-gil-release-threshold.md`) left a
follow-up: the cold path needs 5–10 GB/s SIMD.

Two further facts were discovered while doing this work:

1. **Every ready-made SIMD transcoder is unusable here.** The SIMD in
   `encoding_rs` (`simd-accel`) is gated on nightly-only `core::simd`
   and silently stays scalar on stable Rust; `xutf` requires nightly
   and is permissive (it passes lone surrogates through instead of
   raising); `simdutf` is C++ and would add a C++ toolchain plus a
   dynamic libstdc++ dependency to every wheel built by ADR-0005's
   matrix.
2. **The PEP 393 offsets were hard-coded and wrong for half the
   supported interpreters.** `PyObject` is 16 bytes on a GIL build
   but 32 on a free-threaded one (3.13t/3.14t), and
   `PyCompactUnicodeObject` carried an extra `wstr_length` before
   3.12. The hard-coded layout pointed at the refcount on
   free-threaded builds, where the "compact" bit was then read from a
   refcount word: the fast path silently degraded, and on a
   differently-aligned build it could have returned a bogus slice.
   `Py_GIL_DISABLED` also widens `state.interned` from a 2-bit field
   to a whole `unsigned char`, which moves `kind`/`compact`/`ascii` up
   by 6 bits.

## Decision

1. **Hand-written AVX2 kernel** in `src/utf8/simd.rs`, no new
   dependency. `std::arch::x86_64` intrinsics with runtime
   `is_x86_feature_detected!` dispatch (cached in a `OnceLock`) for
   `avx2 + bmi2`; every other architecture keeps the scalar encoder,
   unchanged. The kernel is only entered through that gate.
2. **One kernel for all three UCS kinds.** Units are widened to eight
   `u32` lanes (free for UCS-1/2: no surrogate-pair decoding — CPython
   picks the storage kind by maximum code point, so a compact UCS-2
   string can never contain a valid pair, and every surrogate unit in
   it is a lone surrogate and an encoding error). Per chunk the
   per-lane UTF-8 length is derived as a 3-bit sum into a 4-bit field;
   if all eight lanes share a length, a fixed shuffle emits them
   directly, otherwise the four candidate bytes of each lane are
   staged and compacted by a `pshufb` whose control comes from a
   256-row table indexed by a half-chunk's eight length bits.
3. **Table-driven compaction rather than arithmetic control
   synthesis.** Building `pshufb` controls with BMI2 `pext`/`pdep`
   costs two general-purpose-register round trips per half and
   measured 1.6–2× slower than one indexed load (see the session
   report's kernel table; the same shape simdutf uses for its
   1|2-byte Latin-1 kernel).
4. **Error semantics unchanged.** Surrogate and out-of-range lanes are
   detected in bulk before any SIMD work and the whole 8-unit chunk
   goes to the scalar encoder, which raises `UnicodeEncodeError` at
   the first bad unit — the same first error a fully scalar pass
   reports.
5. **The object layout is detected and validated at runtime,** not
   compiled in: the `length` field is located by matching the known
   lengths of two long ASCII probe strings against aligned offsets
   inside the object, the remaining fields follow from the fixed
   PEP 393 order, and the candidate layout is accepted only if the
   `state` bits, the ASCII data, a CPython-warmed UTF-8 cache and a
   UCS-2 string's units all match `PyUnicode_AsUTF8AndSize`'s
   behaviour. The pre-3.12 shape is not just the non-ASCII data
   moving: the base object is eight bytes longer, so `utf8_length`,
   `utf8` and even the ASCII data sit eight bytes further out, and the
   non-ASCII data sixteen. The candidate layout models that with one
   `extra` parameter (0 or 8) and lets validation pick; hard-coding
   "the ASCII data is always at `length + 24`" made detection fail on
   3.10 and 3.11, which silently disabled the prefilter there.
   Detection is cached per process. **If it fails, every
   `str` haystack takes `to_cow`** — CPython's encoder, i.e. exactly
   the pre-optimization behaviour. A future CPython layout change
   therefore degrades to "slow", never to "wrong".

## Consequences

- Cold non-ASCII `str` scans went from ×3.0 to ×1.6 of `re`
  (Telegram, GIL, 1 thread: 1.46 s → 0.77 s); free-threaded 1.47 s →
  0.84 s. The kernel measures 1.4–3.8 GB/s against the scalar
  encoder's 0.48 GB/s on the shapes that matter (mixed Arabic 1.9,
  pure CJK 3.8, emoji 1.5).
- rurex now contains `unsafe` code, which `architecture.md` requires
  an ADR for. The unsafe surface is confined to `src/utf8.rs` +
  `src/utf8/simd.rs` and is guarded by differential tests against
  CPython's encoder (torture corpus, seeded fuzz over raw code points
  including lone surrogates, every chunk-boundary and uniform shape).
- The fast path is architecture-specific: x86_64 with AVX2+BMI2 today.
  AVX-512 and aarch64 NEON kernels and folding the transcode into the
  detached scan region for ≥ 4 KB haystacks are deliberate follow-ups,
  not prerequisites.

## Alternatives rejected

- `encoding_rs` with `simd-accel`: no SIMD on stable Rust; the feature
  only opts into nightly-only `core::simd`.
- `xutf`: nightly-only, and permissive — it cannot express the
  `UnicodeEncodeError` contract.
- `simdutf` (C++): fastest, but adds a C++ toolchain and a dynamic
  libstdc++ dependency to every wheel in ADR-0005's matrix. Kept as the
  fallback if the hand-written kernel ever proves insufficient.
- Hard-coded PEP 393 offsets: silently wrong on free-threaded builds
  and before 3.12 (see Context).
- Populating CPython's UTF-8 cache from rurex: the buffer is
  `PyMem_Malloc`-owned and only `PyUnicode_AsUTF8AndSize` may write it
  under its own allocator rules.
- Per-chunk out-of-line helper calls: they spill the whole chunk to the
  stack and cost an order of magnitude (measured 0.05 GB/s vs 2–4 GB/s
  with everything folded into one `#[target_feature]` function).
