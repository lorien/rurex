# Architecture

## Components

The repository is a single Rust crate that produces one CPython extension
module, `rurex`.

    regex (crates.io)          the engine
        |
    src/regex.rs  Regex        Python-visible type; owns a compiled program
        |                     (regex::Regex for str, regex::bytes::Regex for bytes)
    src/regexset.rs  RegexSet  multi-pattern membership in one scan
    src/match_.rs  Match       span/group view over one match
    src/prefilter.rs           lazily derived first-byte prefilter for cold str
    src/flags.rs               flag bit constants -> RegexBuilder options
    src/error.rs               regex::Error -> rurex.error (Exception subclass)
        |
    src/lib.rs  #[pymodule]    module init, module-level functions (compile, escape)

## Compilation path

`rurex.compile(pattern, flags=0)` validates inputs in Python types,
maps flag bits to `regex::RegexBuilder` options in `src/flags.rs`
(mirroring CPython's `fix_flags`, including its error messages and
the `UNICODE`-forced-on normalization for `str` patterns), and builds
the compiled regex once, storing it inside the `Regex` pyclass as a
Rust struct field. Bytes patterns are built with the engine's Unicode
mode off, so classes are ASCII-only single bytes exactly like `re`.

The pattern text first goes through the cheap textual passes that
enforce `re`'s own rejections: leading global inline flags are folded
into flag bits, scoped `(?a:` groups are rewritten to the engine's
`(?-u:`, group names are checked against `re`'s rules, and stacked
quantifiers are refused. Those run on every pattern, accepted or not,
because they reject constructs the engine would accept. The engine then
builds the program, which parses the pattern itself — that is the
compile path's only parse. The `regex-syntax` pass (the engine's own
parser, under the same flag configuration) runs only when the engine
rejects the pattern, so syntax errors still carry structured positions;
`src/error.rs` turns them into `rurex.error` instances with the full
`re.error` attribute set (`msg`, `pattern`, `pos`, `lineno`, `colno`).
A rejection that the validator accepts is an engine limit (compiled
program too large) and maps to `rurex.error` with `None` positions.
Parsing every accepted pattern twice is what made rurex 1.4–2.5× the
engine's compile time; [adr/0015-single-parse-compile.md](adr/0015-single-parse-compile.md)
records the ordering.

Each thread also keeps a one-entry front for the module cache: the
most recent (pattern bytes, flags, handle) is compared by slice, with
no lock, no hash and no key allocation, and a generation counter bumped
by `purge()` invalidates it everywhere. Module functions call the same
Rust implementation as the `Regex` methods instead of dispatching
through Python, so a module call costs a handful of instructions more
than a compiled-pattern call rather than a method lookup.

`rurex.compile_set` does the same for `regex::RegexSet`,
storing one program that answers membership for all patterns in a
single scan. Compilation cost is paid once; matching methods reuse the
stored program.

## Matching path

- `Regex` holds the compiled engine; it is immutable after construction.
- Text inputs are accepted as `str` (Unicode, `regex::Regex`) or `bytes`
  (`regex::bytes::Regex`). Which engine is used is fixed at compile time by
  the pattern's/flag's character mode, not per call.
- Matching calls release the GIL around the scan when the window is
  large enough to pay for the handoff (`py.detach` above the measured
  4 KB threshold): the borrowed text reference is obtained before the
  release and no Python objects are touched inside the closure. Short
  scans stay GIL-held, matching `re`'s own behavior and avoiding a
  lock convoy across threads. This gates single-window scans
  (`search`, `match`, `fullmatch`, `findall`, `is_match`); the
  per-match loops (`finditer`, the scanners, `split`, `sub`) hold the
  GIL per step, because a step normally ends a few bytes past the
  previous match and a handoff cannot be paid back (see `per_step`).
- Results are converted back to Python objects (`Match`, tuples, lists)
  after the scan, under the GIL.
- On free-threaded CPython a whole-scan `findall` is the one workload
  that does not scale: stdlib `re`'s `findall` keeps scaling (208 -> 195
  ns per 470-byte post from one to four workers) while rurex's degrades
  (167 -> 302 ns), because the per-value Python objects sit on CPython's
  shared reference-count paths. The engine does scale (`is_match`:
  1.04× at four workers), so the remaining lever is fewer Python objects
  per call, which is an API question (`plan.md`).
- Every thread matches with its own clone of the engine (one per
  pattern, in the same thread-local table as the capture scratch, keyed
  by the never-reused pattern id). A shared `regex::Regex` serves all
  threads but its owner through its internal cache pool, and that mutex
  is what stopped short-haystack scans from scaling: 134 ns per
  `is_match` at four free-threaded workers against 89 ns for a clone
  whose pool the thread owns (ADR-0019). A clone shares the immutable
  program, so matching behaviour is identical.
- The engine's capture locations are reused: each thread keeps up to
  four `CaptureLocations` keyed by a never-reused pattern id
  (`src/regex.rs`), so a `search`, a `finditer` step, a scanner step, a
  `sub` step or a grouped `findall` match fills scratch instead of
  allocating a `Captures`. The pattern's id is assigned at compile time
  from a global counter, because keying by address would let a freed
  pattern's slot be reused (see
  [adr/0017-thread-local-capture-locations.md](adr/0017-thread-local-capture-locations.md)).
- A `Match` holds its group spans inline for up to four groups (group 0
  plus three capture groups) and spills to a heap vector above that, so
  the usual `finditer` step allocates nothing for spans. The shared
  capture-name table exists only for patterns that actually name a
  group, so an unnamed pattern's match pays no refcount bump either.
  Group text is produced with `PyUnicode_Substring` (character indices)
  or `PyBytes_FromStringAndSize` (byte indices) rather than a
  Python-level slice, and is clamped to the haystack first, because a
  bytes-like haystack can be resized between the match and a later
  `group()` read. See
  [adr/0016-per-match-object-cost.md](adr/0016-per-match-object-cost.md).
- `count` is the counting shape of the same whole-scan path: one Rust
  pass with `find_at` (group 0 only), no `Match`, no capture group and
  no result value, one integer at the end — the only entry point whose
  cost does not grow with the number of matches (ADR-0020).
- `findall` is a dedicated whole-scan path: every match's spans are
  collected in one flat buffer in a single Rust pass (no `Captures`
  object and no per-match `Vec`; patterns without groups use
  `find_at`), then only the returned values (strings, or one string
  per group) are built. No `Match` objects are constructed.
- The zero-width must-advance retry engine is built on the first empty
  match, so a pattern that cannot match empty never produces one and
  never pays for it.
- Work proportional to the whole haystack must not run per call or
  per match: `Match.endpos` for `endpos=None` resolves lazily on
  first read (counting characters is O(n) in the text), capture
  names are shared `Arc` data, and `pos`/`endpos` window conversion
  walks the text at most once, only as far as the indices given.
- Byte-to-character span conversion runs per returned `Match`, so it
  is in place: the group spans the engine filled are rewritten with a
  single sorted-offset walk into a stack buffer (heap only above 16
  groups), with no hash map and no second vector. For a compact ASCII
  `str` the PEP 393 `state.ascii` bit already tells the result path
  that byte offsets equal character offsets, and conversion is
  skipped outright (`utf8::is_ascii_obj`). Per-match loops pass the
  previous match's (byte, char) position as the walk's starting
  point, so each step counts only the characters it advanced over
  instead of re-walking the haystack from its start.

## Error handling

All fallible paths return `PyResult`. Rust panics must not cross into
Python: panicking code paths are bugs to fix, not to map. `regex::Error`
is the only expected failure at the boundary and always maps to
`rurex.error`.

## Memory and safety

`unsafe` is confined to the `str` haystack fast path (`src/utf8.rs`,
`src/utf8/simd.rs`, and the UCS scan in `src/prefilter.rs`), authorized
by [adr/0012-simd-transcoder-and-detected-layout.md](adr/0012-simd-transcoder-and-detected-layout.md),
[adr/0013-cold-str-required-prefix-prefilter.md](adr/0013-cold-str-required-prefix-prefilter.md),
and [adr/0014-gil-released-transcode.md](adr/0014-gil-released-transcode.md).
Everything else relies on PyO3 and `regex` handling their own
internals. Wrapper types own their data; nothing borrowed from Python
outlives a call except through PyO3's checked lifetimes.

The `str` fast path reads PEP 393 fields through raw pointers, so it
depends on an object layout that differs between interpreters (GIL vs
free-threaded object headers, `wstr_length` before 3.12, the widened
`state.interned` on free-threaded builds). That layout is **detected
and validated once per process** at first use; if validation fails,
the module falls back to `PyUnicode_AsUTF8AndSize` and behaves exactly
as if the fast path did not exist. Never hard-code those offsets.

## Text transcoding

`re` never converts: it matches UCS directly. rurex's engine matches
UTF-8, so a `str` haystack must be viewed as UTF-8, in three ways
(`src/utf8.rs`):

1. compact ASCII — the UCS-1 data is already valid UTF-8, borrowed
   zero-copy;
2. non-ASCII whose CPython UTF-8 cache is warm — borrowed zero-copy
   (rurex never writes that cache: it is `PyMem_Malloc`-owned);
3. non-ASCII with a cold cache — first checked against the pattern's
   required first byte (below); only if that byte occurs is it
   transcoded into an owned buffer, by the AVX2 kernel where available
   (x86_64 with AVX2+BMI2, 1.4–3.8 GB/s) and by a scalar loop
   everywhere else (~0.48 GB/s). For a source of at least 32 KB the
   GIL is released around the transcode, so other Python threads can
   run during the encode; shorter sources stay GIL-held because the
   handoff would cost more than the encode (ADR-0014).

The prefilter is derived from `regex-syntax`'s prefix-literal
extractor on the first cold transcode, not at compile time, and cached
on the pattern: a pattern that is only ever matched against ASCII or
warm-cache strings never pays for the parse it would need (ADR-0015).
When every possible match starts with one of at most three distinct
ASCII bytes, that byte is searched in the raw UCS storage (with
unit-alignment checks for UCS-2/UCS-4). Its absence means no match can
exist, so the query returns its no-match answer without transcoding.
The prefilter is only consulted on the cold path; see
[adr/0013-cold-str-required-prefix-prefilter.md](adr/0013-cold-str-required-prefix-prefilter.md).

The detach is safe because `utf8_of_filtered` holds the
`Bound<PyString>` for the whole call and `str` is immutable: the UCS
buffer cannot move or be freed while another Python thread runs. The
raw pointer crosses the detach as a `usize` because raw pointers are
not `Ungil`; the scratch still comes from the calling thread's pool.

The transcoded buffer lives for the duration of the call (`HayKeep`),
so re-scans of a string only rurex touched re-transcode each time —
cheap at those rates, and traded deliberately against a side cache
that would have to stay correct across `str` identity and lifetime.
