# Cold-str required-prefix prefilter

Date: 2026-10-18
Status: accepted (prefilter derivation timing revised by ADR-0015:
computed on the first cold transcode, not at compile time)

## Context

`str` haystacks must be transcoded to UTF-8 before the engine can scan
them. On cold non-ASCII strings that transcode is the dominant cost:
on a 300k-post Telegram sample it accounted for about 38% of `findall`
time (0.31 µs of 0.81 µs per post). ADR-0012 made the transcode itself
3–8× faster, but memory traffic on the string's own bytes bounds how
far that can go.

Most strings cannot match at all. 77.2% of the Telegram posts contain
no `@`, the required first character of the benchmark pattern. The
transcoder is paid before the engine can discover that. Skipping it
requires a source-side test that is both cheap and certainly
conservative: a false "impossible" answer would silently drop matches.

## Decision

1. **Use `regex-syntax`'s literal extractor, not a hand-written HIR
   walk.** `src/prefilter.rs` parses the adjusted pattern with the same
   configuration as the validator and runs
   `hir::literal::Extractor` in prefix mode. Its contract guarantees
   that every match starts with one of the returned literals. The
   optimization accepts a sequence only when it is finite, non-empty,
   and every literal is non-empty and starts with an ASCII byte; the
   distinct first bytes are capped at three so the `memchr` family can
   search for any of them.
2. **Scan the PEP 393 UCS storage, not a copy.** For UCS-1 the raw
   bytes are searched directly. For UCS-2/UCS-4 the raw bytes are
   searched with `memchr`, and each hit is checked against the unit
   alignment and the zero bytes of the unit, so `0x40` inside an
   unrelated code unit is not mistaken for ASCII `@`. This works on
   both endiannesses and avoids a scalar per-unit loop. A cap on
   false hits (64) falls back to "may match" so a pathological input
   cannot make the prefilter slower than the transcode it avoids.
3. **Gate it to the cold-transcode path.** ASCII strings, strings with
   a warm CPython UTF-8 cache, legacy objects without a validated
   layout, and bytes-mode haystacks never consult it. If the required
   byte is absent, the operation returns its normal no-match answer
   without transcoding or scanning: empty list, exhausted iterator,
   `None`, or `False`.
4. **Be conservative at compile time.** Infinite literal sequences,
   empty literals, non-ASCII first bytes, NUL, more than three distinct
   first bytes, or a parse failure all disable the prefilter. The
   prefilter is an optimization, never a correctness dependency.
5. **Surrogate behavior follows `re` on the skipped path.** A cold
   non-ASCII string containing a lone surrogate and no required byte
   returns the no-match answer instead of raising `UnicodeEncodeError`;
   `re` also returns a normal result because it never encodes. Strings
   that do reach the transcoder still raise, so full surrogate parity
   is a separate task in `plan.md`.
6. **The query paths opt in explicitly.** `findall`, `finditer`,
   `search`, `match`/`prefixmatch`, `fullmatch`, and `is_match` use the
   filtered haystack. `split`/`sub`/`subn` keep the unfiltered path for
   now: their no-match answers must reproduce the current `str`/`bytes`
   copy and subclass behavior, which is separate work.

## Consequences

- Telegram GIL single-thread `str` moved from 0.75 s to about 0.53 s,
  reaching `re` parity (about 0.52 s on the same run); the apparent
  transcode cost dropped from 0.31 µs to 0.04 µs per post on the 300k
  sample. Bytes mode is untouched by the prefilter but gains from the
  separate `findall` allocation work.
- The unsafe surface grows by one read of the UCS buffer, under the
  same detected-and-validated layout contract as ADR-0012. The scan
  has unit tests for UCS-1/2/4 alignment, endianness, multi-byte sets,
  and a differential check that a `false` answer implies the required
  byte is absent.
- The prefilter is internal and cached on the pattern (ADR-0015): no
  new public API, no side cache, no change to `Match` offsets or
  result values.

## Alternatives rejected

- Hand-written HIR traversal for "required first unit": duplicates
  parser semantics (flags, case folding, classes) and is much easier to
  get subtly wrong than the extractor that the regex engine itself
  uses.
- Scalar per-unit scan: about 0.1–0.2 µs per 322-unit post on this CPU,
  consuming most of the transcode saving.
- SIMD well-formedness check so lone surrogates keep raising: would
  recover the old error on the skipped path, but `re` does not raise
  there; it is extra machinery toward a divergence the API does not
  want.
- Applying the prefilter to warm UTF-8 or bytes: the engine's own
  prefilter already scans those haystacks; the duplicate check only
  saves call setup.
- A side cache of transcoded UTF-8: has to stay correct across `str`
  identity, lifetime, and mutability, for a workload that scans each
  string once.
