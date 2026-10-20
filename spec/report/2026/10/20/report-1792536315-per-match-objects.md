## Report on task: Cheaper per-match objects for finditer-heavy workloads

### Task (archived from plan.md)

## Cheaper per-match objects for finditer-heavy workloads

Status: done

rebar's `08-words/all-english` (byte mode, 56,601 matches in 2,500
subtitle lines) measures 3.56 ms for rurex against 2.25 ms for stdlib
`re` and 0.71 ms for the `rust/regex` engine alone. The gap is not the
scan: with that many matches, the cost is one Python object per match,
about 63 ns per match for rurex against 40 ns for `re` and 13 ns for the
engine's own loop. Every `finditer`/`count`-shaped workload pays it, and
it is the largest single reason rurex trails `re` on a benchmark family
that is otherwise scan-bound.

`Match` currently allocates a `Vec` for its spans (plus the pyclass
itself) and clones an `Arc` of capture names. The candidate fixes are
those already noted in the result-path report: an inline array for the
usual 0–3 groups with a heap fallback, and a thread-local span buffer
returned on drop, measured against rebar's words benchmarks and the
existing `finditer` microbenchmarks.

Not part of this task: the Russian variants of that group. There the
engine itself is slower than `re` (6.07 ms against 3.03 ms) because its
DFA cannot handle a Unicode-aware `\b` on non-ASCII bytes, and rurex's
transcode of the 130 KB haystack is under 1% of the call. That is an
upstream engine limitation, recorded on the rebar page, not something
the binding can fix.

References: ADR-0008,
`spec/report/2026/10/18/report-1792364009-result-path-offsets.md`,
`spec/report/2026/10/19/report-1792446048-rebar-comparison.md`

### Done

- **Measured the cost before changing anything.** A per-component
  decomposition on the benchmark host (same haystack and pattern,
  leased core, pinned CPU, `min` of seven runs) showed the plan's
  premise was wrong: `Match` construction was already within ~2 ns of
  `re`, and the span `Vec` was not measurable at all. The gap was the
  group *accessor*, which went through a Python-level slice
  (`PySlice` + `__getitem__` dispatch). Numbers in ADR-0016; per match,
  text mode: iterate-only 30.7 ns (`re`) / 33.1 (rurex), `span(0)`
  40.1 / 52.9, `group(0)` 39.1 / 68.7.
- **`Spans`** (`src/match_.rs`): group spans live inline for four slots
  (group 0 plus three captures) and spill to a `Vec` above that, read
  through `Deref`, so no consumer knows which representation is in
  use. `captures_spans_text`/`_bytes`, the zero-width retry engine
  (`src/retry.rs`), `step_source`/`cpython_step` and `offset_spans` all
  produce or consume `Spans` instead of a `Vec`.
- **Group text via the direct constructors**: `PyUnicode_Substring`
  for `str` (character indices — exactly what a text span is) and
  `PyBytes_FromStringAndSize` over the haystack buffer for `bytes`,
  both replacing the `PySlice` round trip. Whole-string spans of an
  exact `str`/`bytes` still return the stored object (CPython slices
  that way, and `re` does too); a subclass haystack gets a plain copy.
- **Span clamping**: `clamp_span` bounds the span to the haystack the
  way Python slicing does. The first version omitted it and the
  vendored CPython suite caught it immediately — `test_bug_29444`
  resizes a `bytearray` between the match and `group()`, and `re`
  returns the clamped current contents. The Rust slice panicked there;
  now both modes clamp before constructing.
- **Name table only when a group is named**: `Regex::names` and
  `Match::names` became `Option<Arc<Vec<Option<String>>>>`, so an
  unnamed pattern builds no table and a `Match` does no refcount bump.
- **Leaner `group()`**: no value vector or iterator for the
  single-argument case (the hot shape).
- Full verification: `cargo test` (45), `make test-all` (pytest 439 +
  CPython `test_re.py` + the rust-regex corpus + the divergence corpus,
  each in its own process), plus direct `re`-versus-rurex checks on
  astral characters, UCS-2 text, lone surrogates, `str`/`bytes`
  subclasses and bytes-like haystacks.
- Republished the rebar page from a full curated + dictionary run with
  the new wheel.

### Results

Controlled A/B on the benchmark host: three alternating runs per wheel,
same core lease and CPU, `-f '^curated/08-words'`, `--max-time 3s
--max-iters 10`, `python/rurex` only. Before = wheel built from commit
e0bdfa8 (sha256 85fadc2b…), after = this work tree (sha256 8bba10b5…).

| benchmark | before | after | change |
|---|---|---|---|
| `08-words/all-english` | 3.64 / 3.65 / 3.65 ms | 2.77 / 2.74 / 2.78 ms | **−24%** |
| `08-words/all-russian` | 9.78 / 9.78 / 9.73 ms | 9.12 / 9.17 / 9.14 ms | −6.5% |
| `08-words/long-english` | 115.6 / 112.8 / 115.1 µs | 111.2 / 111.4 / 108.6 µs | −3% |
| `08-words/long-russian` | 3.54 / 3.54 / 3.53 ms | 3.49 / 3.48 / 3.47 ms | −1.7% |

The gain is in text mode, which is the mode `python/rurex` runs: the
ASCII haystack is borrowed rather than transcoded, but `m.group(0)`
returns `str` pieces, and that is the path the direct constructor
replaces. My first ad-hoc decomposition used a bytes pattern and showed
no delta at all, which is why the A/B above exists — rebar's model is
the instrument that decided it.

Republished curated run: `08-words/all-english` 3.68 → 2.76 ms (now
1.23× behind `re`, was 1.63×), `all-russian`
10.05 → 9.20 ms; the `08-words` group ratio drops from ×1.78 to ×1.62
and rurex's overall score from ×2.72 to ×2.65.

Per-match cost is now about 49 ns against `re`'s 40, with the engine's
own loop at 12.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0016-per-match-object-cost.md`: inline
  spans, direct `str`/`bytes` group construction, span clamping, the
  names table only for named groups, and why the thread-local span pool
  (the plan's second candidate) cannot work — a `Match` outlives the
  iteration step, so it cannot borrow a thread-local buffer.
- [acted] `spec/docs/architecture.md` "Matching path" now documents the
  per-match storage and accessor design and cites ADR-0016.
- [acted] `spec/docs/index.md` catalogues ADR-0016.
- [acted] `docs/rebar-benchmark.md`/`.html` and the charts were
  regenerated; the "Match-heavy iteration" commentary no longer
  hard-codes "63 ns / 40 ns / 13 ns" (it derives the per-match cost
  from the measured medians) and no longer calls the point "byte mode"
  — it is an ASCII haystack in text mode.
- [acted] `benchmarks/rebar_run.py --rurex-build` (added in the previous
  session) carries the wheel provenance into the page; this run used it.

### Future-task notes

- [open] The remaining per-match gap is per-call machinery: a
  `#[pymethods]` entry point is a `METH_VARARGS` wrapper with `Bound`
  plumbing, worth about 10–15 ns against a C method table. Closing it
  means implementing `Match` as a raw `PyTypeObject` with hand-written
  slots, which ADR-0016 rejects. Revisit only if per-call overhead shows
  up as the dominant term on a benchmark that is otherwise at parity.
- [open] `sub` with a callable replacement clones the match's spans per
  call (`spans.clone()`), because the loop needs them after the `Match`
  is built. Below five groups that is now an allocation-free copy; a
  pattern with more groups still clones a `Vec` per match. Reordering
  the advance computation before `Match::new` would remove it.
- [open] `Match` still carries `pos` and `endpos` (32 bytes) for
  `m.pos`/`m.endpos`, which no measured workload reads. A sentinel or a
  compressed `Endpos` would shrink the object; measurement says the
  object's size is not a cost, so this waits for evidence.
- [open] `07-unicode-character-data/parse-line` and other text-mode
  `count`/`count-captures` points also improved, but their remaining
  loss is dominated by the engine's Unicode word-boundary path, not by
  the binding.

### Tooling/process

- A per-component decomposition is not a substitute for the benchmark:
  my ad-hoc script measured bytes mode and reported "no change" while
  rebar measured a 24% win in text mode. When an optimization targets a
  rebar point, A/B the wheels through rebar (alternating, several runs,
  same lease) and use the decomposition only to explain *why*.
- Installing two wheels alternately in one lock session is the cheap
  way to A/B: `pip install --force-reinstall --no-deps` per wheel, one
  `rebar measure` per wheel per round, then compare medians per point.
- The vendored CPython conformance suite earned its keep: a
  `bytearray`-mutation test (`test_bug_29444`) caught a Rust panic that
  no rurex test covered. Any change that turns a Python-level operation
  into a direct buffer read has to clamp, because bytes-like haystacks
  are live buffers.
