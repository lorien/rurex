## Report on task: Full lone-surrogate parity for str haystacks

### Task (archived from plan.md)

## Full lone-surrogate parity for str haystacks

Status: done

rurex transcodes non-ASCII `str` haystacks to UTF-8. CPython's encoder
rejects lone surrogates (U+D800..U+DFFF), so rurex raises
`UnicodeEncodeError` where stdlib `re` matches the in-memory string
normally. The cold-str prefilter returns the normal no-match answer
when the required first byte is absent, but haystacks that reach the
transcoder still raise. Choose and implement full parity: keep
surrogates through matching without corrupting offsets, or add a
UCS-aware path; add differential tests against `re`.

References: ADR-0012, ADR-0013

### Done

- Chose the stand-in substitution over a UCS-aware engine path. The
  engine (`regex` crate) accepts only `&str`/`&[u8]` and cannot match a
  lone surrogate at all — WTF-8 surrogate bytes fail `.` and every
  class — so a real alternative would mean forking `regex-syntax` and
  `regex-automata`, which upstream's maintainer calls "quite a lot of
  work". The substitution rests on a property verified against `re`:
  private-use (Co) code points are indistinguishable from surrogates
  (Cs) for the entire class vocabulary — not `\w`, `\d`, `\s`, matched
  by `.` and negated classes, no case mapping, identical casefold. The
  block U+F0000..U+F07FF is exactly 2048 wide, so the mapping is one
  code point per code point and every offset path (`spans_to_py`,
  `window_bytes`, `finditer` counters) stays valid untouched.
- `src/utf8.rs`: `SURROGATE_BASE`, `surrogate_stand_in`,
  `in_stand_in_block`; the scalar encoder and the SIMD kernel's
  scalar hand-off map a surrogate instead of raising when the pattern
  allows it (threaded as a `&mut bool` so a UCS-4 haystack that
  contains a real stand-in character stops mapping — the two would be
  indistinguishable). `Utf8Scratch` records `mapped` so the result path
  knows whether it must restore anything.
- `src/regex.rs`: `Regex::stand_in` (and `RegexSet::stand_in`) computed
  by `utf8::pattern_allows_stand_in`, which opts out when the pattern
  names a stand-in code point (literal or `\x{...}`) or asks for
  `\p{Co}`/`\p{Cs}`. `Hay::Text` carries the `mapped` flag;
  `text_result` rebuilds result strings through
  `PyUnicode_DecodeUTF16(..., "surrogatepass")` only for slices that
  contain a stand-in, so `findall`, `split`, `sub`, `subn` and
  `Scanner::scan` return the original text. `Match.group()` already
  sliced the original object and needed no change.
- `FindIter` and the SRE scanner now obtain the haystack through
  `utf8_of` instead of `extract::<&str>()`, keeping the transcoded
  buffer for the iterator's lifetime; they use one extraction per step
  on the cheap borrow path so iteration speed is unchanged (measured
  2.19 ms against 2.43 ms before, same document and machine state).
- Surrogate-free strings are untouched: the kernel already detected
  surrogates in bulk for the old error path, and the new code only
  changes what the scalar hand-off writes. `findall` on short posts
  costs about 1.5–2% more instructions from the extra plumbing
  (`Hay` flag, one more argument, the result helper's branch).
- `tests/test_surrogates.py`: 130 differential cases against `re` over
  nine haystacks (UCS-2 and UCS-4 storage, leading/trailing/adjacent
  surrogates, mixed with astral characters) and eleven patterns across
  `findall`, `search`, `match`, `fullmatch`, `finditer`, `split`,
  `sub`, `subn` and `Scanner`; plus the three residual corners.

Checks: 439 pytest (309 existing + 130 new), 3,484 conformance, 45 Rust
tests, clippy and fmt clean.

### Spec/ADR amendments

- [acted] `spec/docs/api.md` has a "Lone surrogates" entry under
  "Semantic differences from `re`": the mapping, why offsets are
  unchanged, and the three corners that still raise (haystack already
  contains a stand-in character; pattern names the block or asks for
  `\p{Co}`/`\p{Cs}`; the pattern itself contains a surrogate).
- [acted] No new ADR: no dependency, no public API change, and the
  unsafe surface is unchanged. ADR-0012's "surrogate lanes raise"
  decision 4 is narrowed to "raise when the pattern cannot be matched
  safely", which `api.md` now states.

### Future-task notes

- [open] Pattern-side parity: a pattern *literal* containing a
  surrogate still fails with `TypeError: invalid str pattern` (it goes
  through `extract::<&str>` in `Regex::build`), and `\ud800` still
  fails as an escape. Fixing it needs the same stand-in mapping applied
  to the pattern text before compilation, plus `Pattern.pattern` stored
  as the original `PyString` so it can round-trip. Tracked separately;
  the haystack task deliberately did not widen into it.
- [open] The stand-in block is fixed, so a haystack containing
  U+F0000..U+F07FF plus a surrogate raises. Choosing a free block per
  haystack would need a compiled pattern variant per block; the corner
  is documented and tested instead.
- [open] SIMD mapping for surrogate lanes: today a chunk containing a
  surrogate goes through the scalar hand-off, which is fine for the
  rare case but could be vectorised if surrogate-bearing strings ever
  become common.

### Tooling/process

- `cargo clippy --all-targets` catches test-only tuple-pattern breakage
  that `cargo test` reports separately; both needed updating when the
  variants gained a field.
- A/B measuring the working tree against the stashed tree with
  `make build-release` between them is the only reliable way to tell a
  real regression from machine noise here: the same `finditer` probe
  read 1.92 ms, 2.43 ms and 2.19 ms across conditions, and only the
  paired runs were trustworthy.
