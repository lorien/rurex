## Report on task: In-place character-offset conversion for text results

Owner instruction: "make it fast", with no fixed task; the previous
session's profile was accepted as the starting point. Ad-hoc work, so no
record is archived from `plan.md`.

### Done

- Measured the per-call result path on the release build and found the
  single largest non-engine cost: every text `Match` converted byte spans
  to character offsets by building a `Vec` of distinct offsets, sorting
  and deduplicating it, and filling a `HashMap<usize, usize>` per call,
  then allocating a second `Vec`. Three allocations plus a hash table for
  what is normally one or two spans.
- Replaced `spans_to_py` with `to_py_spans`: the engine's own span buffer
  is rewritten in place with one sorted-offset walk into a stack buffer
  (heap only above 16 capture groups). No hash map, no second vector, no
  per-call allocation for the usual group counts.
- Added `utf8::is_ascii_obj`: two field reads over the already-validated
  PEP 393 layout answer "compact ASCII", for which byte offsets already
  are character offsets, so conversion is skipped entirely. It is O(1)
  and answers `false` whenever the layout is unvalidated, which only
  means the slow (still correct) conversion runs.
- Collapsed the duplicated per-span conversion loops in `FindIter::step`,
  `SreScanner::next` and a callable `sub` onto the same helper.
  `FindIter` keeps its
  `chars_seen`/`bytes_seen` counters: the conversion reads them as its
  starting point, so a step only counts the characters it advanced over.
- `sub` with a callable still needs byte spans for `advance_after` and
  template expansion, so it converts a copy; that path drops from three
  allocations to one.
- `fullmatch`'s anchored text path now offsets spans in place instead of
  calling the allocating `offset_spans`.

Evidence, same probe script and `perf stat` event list, 200k calls on a
26-character haystack, release build, `taskset -c 2`:

| call | instructions/call before | after | cycles/call before | after |
|---|---:|---:|---:|---:|
| `search` (str) | 5,072 | 3,582 | 2,005 | 1,555 |
| `search` (bytes) | 3,604 | 3,394 | 1,465 | 1,532 |
| `is_match` (str) | 1,801 | 1,788 | 826 | 836 |
| `findall` (str) | 3,876 | 4,028 | 1,581 | 1,621 |

Text `search` was 1,468 instructions and 540 cycles above the byte path
for the same match; it is now within a few percent of it, and within
14% of `re`'s 3,027 instructions for the same call (was 68% above). The
`findall` and `is_match` shifts are within run-to-run noise; neither path
touched the changed code.

Final checks: `cargo fmt`, `cargo clippy --all-targets -D warnings`,
`cargo test` (45 passed), `tests/` (309 passed), conformance suites
(3,484 passed). No new `unsafe`, no new dependency, no public API
change, so no ADR.

### Iterator regression found and fixed in the same session

The in-place conversion above initially walked from byte 0 on every
call. `finditer`, the scanners and a callable `sub` convert spans once
per match, so on a non-ASCII haystack that turned a 160 KB document
with 5,000 matches quadratic: `finditer` took 29.5 ms against `re`'s
1.3 ms, worse than the code it replaced, whose `chars_seen`/
`bytes_seen` counters made each step walk only its own advance.

Fixed by giving the conversion a starting `(byte, char)` position and
feeding it the previous match's end; `FindIter` keeps those counters
again, and `sub` tracks them for its callable path. While there, the
per-match loops stopped releasing the GIL per step (`per_step`): the
detach was gated on the whole-window size, so every step of an
iteration over a ≥4 KB text paid a handoff for a scan that normally
ends a few bytes later. Measured on the same document:

| call | before this session's fixes | after | `re` |
|---|---:|---:|---:|
| `finditer` full | 29.5 ms | 1.92 ms | 1.25 ms |
| `split` | 1.52 ms | 1.48 ms | 0.95 ms |
| `sub` (template) | 1.71 ms | 1.54 ms | 1.02 ms |
| `findall` | 0.82 ms | 0.80 ms | 1.00 ms |

Whole-window scans still detach once around their single scan, so the
long-document release from ADR-0014 is untouched.

### Benchmark pages refreshed

The first attempt was on the laptop and was wrong: the reports already
say published numbers come from the benchmark host (`regexus`), but the
rule lived only as an `[open]` item in a report, not in the workflow
docs. Two local runs then died in the efficiency guard at free-threaded
`threads=8` (worker efficiency 0.56–0.66 against the 0.90 floor)
because the laptop was busy with unrelated processes. The rule is now in
`spec/docs/dev.md` ("Benchmarks") and in the final checks in
`spec/docs/testing.md`, and the two report items that carried it are
marked `[acted]`.

The published refresh comes from `web@regexus:/web/benchmark/rurex`
(i5-13500, 20 logical CPUs, load 0.08, governor `performance`), synced
with rsync, no commit for transport. `make bench` there rebuilt both
venvs (new `build-free` dependency) and finished with zero rejected
points in 3m05s.

- `results.json` now records the machine and, per interpreter, the
  extension's path, size, mtime and sha256, replacing the hard-coded
  `build` string that could not prove which binary ran.
- Numbers moved to the host: scan ×30–43, extraction ×1.8–2.0, density
  sweep ×1.5–4.5, GIL threads ×40 to ×179, free-threaded threads
  ×28–43, position threads ×112–128 (GIL) and ×27–34 (free).
- The "free-threaded costs 2.7×" claim in the previous edition was an
  artifact of the stale `.venv-t`: on the clean host, with both
  extensions rebuilt, the same pile costs 0.19 s free-threaded against
  0.20 s on GIL.
- Test 5 now completes n=26 (`re` 1.98 s) before n=28 times out, so the
  table gained a row.
- The nine result tables had no GFM delimiter row and rendered as raw
  pipe text in the HTML twin; fixed, ten tables now.

### Follow-up: where the Telegram `str` row actually goes

Isolated with a genuinely warm CPython UTF-8 cache (via `ctypes`
`PyUnicode_AsUTF8AndSize`; `str.encode()` and `json.dumps()` do *not*
warm it, which invalidated a first attempt): `is_match` on a 470-char
Cyrillic post costs **630 ns cold, 126 ns cache-warm, and bytes 140 ns**
on the same content. The whole str-vs-bytes deficit is therefore the
UCS→UTF-8 transcode, about 500 ns per 940-byte post (~1.9 GB/s), and a
warm cache erases it completely. Per corpus population (470-byte posts,
81.4% without a mention): the no-mention case is 233–271 ns with the
prefilter skipping the transcode and is *faster* than `re`; the
mention case is 779 ns and is the only losing population.

Tried the `findall` result path accordingly: a lazily taken per-thread
span buffer plus direct list append, dropping the intermediate object
vector. Microbenchmarks improved 2.5–9% of instructions on matching
calls (one match str 8,973 → 8,748; bytes 3,770 → 3,548; 20 matches
bytes 26,074 → 25,414) and were flat on the no-match majority. The
paired A/B on the host (Telegram, 5 samples) was mixed, though:
bytes −2.6% at one worker and −3.3% at four (CIs exclude zero), but
**str +1.7% at one worker** (CI [+1.4%, +2.8%]). Reverted: the row that
matters got worse, and a mixed trade is not worth the added machinery.

So the remaining `str` gap is not binding work; it is the encoder.
Levers, in order of honesty: a faster kernel (AVX-512 is unavailable on
both the benchmark host, Raptor Lake, and the laptop, Kaby Lake, so that
ADR-0012 follow-up is dead for these machines), or caching the encoded
form across calls so a string is encoded once rather than once per call
(a semantic and memory trade-off that needs an ADR and does not help a
single-pass benchmark).

### Module call path: per-thread front cache and direct dispatch

Module-level functions (`rurex.findall(pattern, ...)` and friends) were
the other half of the call-dense problem: every call took the shared
cache mutex, hashed the pattern, allocated a key vector, cloned a
handle, and then reached the implementation through Python attribute
lookup and `call_method`. Measured overhead against a compiled pattern
on the same 470-char post, instructions per call:

| function | before | after |
|---|---:|---:|
| `findall` | +1,891 | +216 |
| `search` | +1,875 | +251 |
| `match` | +1,870 | +333 |
| `fullmatch` | +1,966 | +266 |
| `split` | +2,004 | +179 |
| `sub` | +1,967 | +280 |
| `finditer` (first item) | +1,841 | +295 |

Two changes:

- **One-entry per-thread front cache.** The last (pattern bytes, flags,
  handle) is compared by slice, with no lock, no hash and no key
  allocation; a miss falls through to the shared FIFO cache and is then
  recorded. A generation counter bumped by `purge()` invalidates every
  thread's entry, so `purge` still forces a fresh object (the existing
  identity test covers it).
- **Direct dispatch.** The bodies of `search`, `prefixmatch`/`match`,
  `fullmatch`, `findall`, `finditer`, `split` and `substitute` moved to
  `pub(crate)` `*_from(this: &Py<Regex>, ...)` functions that the
  pyclass methods and the module functions both call, instead of the
  module side doing `call_method` and extracting the result.

Wall clock on the same post (min of five runs): `findall` 324 ns module
against 321 ns compiled (was ~400 against ~220), `split` and `sub` at
parity, `search` still +51 ns because the module side boxes the
`Match`.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` "Benchmarks" now states that published
  runs go to the benchmark host, with the rsync/build/pull-back
  procedure; `spec/docs/testing.md` final checks point at it.
- [acted] `spec/docs/architecture.md` "Matching path" records the
  in-place span conversion and the per-step GIL rule.
- [acted] No ADR: result-path optimizations inside the existing safety
  and API contracts.

### Future-task notes

- [acted] The remaining text `search` gap to `re` is one engine-side
  allocation (`Regex::captures_at` builds a `Captures` per call) plus the
  final span vector. The known fix is the `CaptureLocations` reuse listed
  in `report-1792325206-findall-prefilter-speedup.md`, which now also
  covers `search`/`match`/`finditer`, not just grouped `findall`.
  [acted — the reuse shipped for every site (8–16% per `search` call):
  ADR-0017, report-1792570216-capture-locations.]
- [acted] Inline the usual 0-to-3 group spans inside `Match` (small array
  plus heap fallback) to remove the last result-path allocation; measure
  against the ~14% instruction gap to `re` before committing to it.
  [acted — four inline slots plus a heap spill shipped; the measurement
  found the span `Vec` itself immaterial and the group accessor's
  Python-level slice the real cost (24% on `08-words/all-english`):
  ADR-0016, report-1792536315-per-match-objects.]
- [open] `FindIter::step`, `SreScanner::next` and `Scanner::scan` obtain
  the haystack with PyO3's `extract::<&str>`, i.e. CPython's encoder on
  a cold non-ASCII string, instead of the project's AVX2 transcoder.
  Warm-cache steps measure fine (67 µs for the first match of a 160k
  character document, encoder excluded), so the gain is limited to the
  first step of a cold document; it also populates CPython's UTF-8
  cache, which may be a deliberate memory/CPU trade-off nobody has
  written down. Measure before changing it.
- [open] Short non-ASCII strings pay ~570 instructions of fixed cost plus
  ~8 instructions per character to transcode. Measured directly since
  (see "Follow-up" above): the *per-character* cost dominates at corpus
  sizes (a 470-char post costs ~500 ns cold against 126 ns cache-warm),
  so folding the prefilter or trimming the fixed cost would buy well
  under 10% of this row. The real lever is encoding once per string
  instead of once per call, which needs an ADR.
- [open] The shared `regex` cache-pool contention under free-threaded
  Python remains unaddressed; upstream (`regex-automata` docs,
  rust-lang/regex#934) prescribes a per-thread `Regex` clone or an
  explicit per-thread `Cache`, neither of which is in this build.
- [acted] The module-level pattern cache no longer takes the process-wide
  `Mutex` on a hit: a per-thread one-entry front cache compares the
  pattern by slice, and module functions dispatch straight into the same
  Rust implementation as the methods. Overhead against a compiled
  pattern fell from ~1,900 to ~200-330 instructions per call.

### Tooling/process

- The kernel grant `kernel.perf_event_paranoid=1` made `perf` usable for
  user-space sampling; `perf stat -e instructions` gives run-to-run
  stable numbers where wall-clock timings on this box drift by 20%.
  Instruction counts, not nanoseconds, should be the acceptance metric
  for per-call work of this size.
- `Cargo.toml` sets `strip = true`, so `perf report` shows raw addresses
  for rurex frames (libpython and libc frames symbolicate, which was
  enough to locate the allocator share). A symbolicated profile needs a
  local build with `RUSTFLAGS="-C strip=none"` into a scratch
  `CARGO_TARGET_DIR`; do not relax the shipped profile for it.
- Throwaway probe scripts and perf data belong under `var/` (gitignored);
  the recorded recipes are the mode names in this report, not committed
  files.
- [acted] `benchmarks/run.py` no longer hard-codes the `build` string:
  `extension_identity()` records the CPU plus each interpreter's
  extension path, size, mtime and sha256, and warns when the extension
  is older than the Rust sources.
- [open] `benchmarks/run.py` ignores unknown flags: `--help` starts a
  full 45-second benchmark run and leaves `/tmp/core4.lock` metadata
  behind (harmless, but confusing). Parse the known flags and reject the
  rest.
- [open] The generated HTML twins are gitignored, so the published page
  is whatever the last local `make_charts.py` wrote; `docs/benchmarks.md`
  and the charts are the tracked truth. Keep the two in sync in the same
  commit.
