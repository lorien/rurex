# Per-match object cost in `finditer`-heavy workloads

Date: 2026-10-20
Status: accepted

## Context

rebar's `count`/`count-spans` models iterate `finditer`, so every
Python object the binding builds per match sits inside the measurement.
On `08-words/all-english` (2,500 subtitle lines, 56,601 matches) rurex
measured 3.65 ms against stdlib `re`'s 2.26 ms and the `rust/regex`
engine's 0.70 ms, i.e. about 64 ns per match against `re`'s 40 ns.

The open task assumed the cost was the per-match heap allocation:
`Match` stored its spans in a `Vec` (one allocation per match, plus the
pyclass itself) and cloned an `Arc` of capture names. Measuring the
components separately (same host, leased core, `min` of seven runs over
the same haystack/pattern) showed that assumption was wrong:

| accessor per match | `re` | rurex (before) |
|---|---:|---:|
| iterate only (`sum(1 for _ in …)`) | 30.7 ns | 33.1 ns |
| `m.span(0)` | 40.1 ns | 52.9 ns |
| `m.group(0)` | 39.1 ns | 68.7 ns |

`Match` construction was already within ~2 ns of `re`; the span `Vec`
was not measurable at all (a 24-byte `tcache` allocation is a few
nanoseconds). The gap was in the group *accessor*: it went through a
Python-level `[start:end]` slice — building a `PySlice` object and
dispatching `__getitem__` — where `re` (a C type) calls
`PyUnicode_Substring`/`PyBytes_FromStringAndSize` directly.

## Decision

1. **Store spans inline for the group counts real patterns have.** A
   `Match` keeps four slots (group 0 plus three capture groups) in the
   object and spills to a `Vec` above that. This does not show up in
   the benchmark, but it removes a malloc/free pair per match from a
   path whose only allocation should be the Python object itself.
2. **Build group text with the direct constructors.** Text mode calls
   `PyUnicode_Substring` (its argument is a character index, which is
   exactly what a text span already is); bytes mode uses
   `PyBytes_FromStringAndSize` over the haystack buffer. Both replace
   a `PySlice` allocation plus `__getitem__` dispatch, which was the
   dominant per-match cost (about 17 ns of the 28 ns the accessor
   cost).
3. **Clamp the span to the haystack before constructing.** `str` and
   `bytes` are immutable, so a span is always in range there; a
   bytes-like haystack (bytearray, memoryview, array) is a live buffer
   that can be resized between the match and a later `group()` read,
   and `re` then returns the clamped current contents. CPython's own
   `test_bug_29444` covers it, and the vendored conformance suite
   caught the first version of this change panicking there.
4. **Build the name table only for patterns that name a group.** The
   `Arc<Vec<Option<String>>>` became an `Option`, so an unnamed pattern
   allocates no table at compile time and a `Match` does no refcount
   bump per match.
5. **Keep whole-string identity.** A span covering the whole haystack of
   an exact `str`/`bytes` returns the stored object itself, because
   CPython's slicing does (`s[:] is s`) and `re` does too; a subclass
   haystack gets a plain copy, again like `re`.

## Consequences

- `08-words/all-english` (text mode) improved from 3.65 ms to 2.77 ms,
  a 24% cut, and `all-russian` by 6.5%; the two `long-*` points (few
  matches, scan-bound) move by under 5%. On the republished curated set
  rurex's score against the fastest engine improves accordingly.
- The remaining per-match gap is per-call machinery, not payload:
  `m.span(0)` still costs rurex 53 ns against `re`'s 40 ns, and
  `m.group(0)` 69 against 39. A PyO3 `#[pymethods]` entry point is a
  `METH_VARARGS` wrapper with `Bound` plumbing; `re.Match` is a C type
  with a hand-written method table. Closing that would mean giving up
  `#[pyclass]` for `Match`, which is not worth the unsafe surface.
- Whole-string spans and subclass haystacks keep `re`'s identity and
  type behaviour; the vendored `test_re.py`, the rust-regex corpus and
  the divergence corpus all pass unchanged.

## Alternatives rejected

- **A thread-local span buffer returned on drop** (the other candidate
  in the plan): a `Match` outlives the iteration step — the user holds
  it — so it cannot borrow a thread-local buffer. Recycling would need
  a `Drop` on every `Match` plus a pool, to replace an allocation that
  measurement could not see.
- **Shrinking the `Match` object instead** (u32 offsets, sentinel
  endpos): the object's size was not the cost either; construction is
  already within 2 ns of `re`.
- **A hand-written `METH_FASTCALL` method table for `Match`**: would
  remove most of the remaining accessor overhead, but PyO3 does not
  support it for `#[pyclass]`, so it means implementing the type as a
  raw `PyTypeObject` and hand-maintaining every slot and reference
  count.
- **Returning a lazy group view** instead of a `str`/`bytes`: not what
  `re` returns, and it moves the copy into the caller's comparisons.
