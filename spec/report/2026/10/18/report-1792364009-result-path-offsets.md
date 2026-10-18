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

`make bench` re-ran the main suite on the same 8-CPU laptop class as
the published page, rewriting `benchmarks/results.json`, the nine
charts and `docs/benchmarks.md` (plus the gitignored
`docs/benchmarks.html` twin). Findings while refreshing:

- The previous edition's rurex numbers were measured on a build about
  2.5× slower: scan ratios move from ×12–13 to ×31–35, the GIL-thread
  pile from 0.80 s to 0.32 s at one worker, while every `re` number
  stays within a few percent (76.7 → 76.6 µs at 64 KB, 10.10 → 9.98 s
  for Test 6). That is the signature of a debug extension left
  installed: the run predates `make bench` forcing the release rebuild
  (commit 87f6f2e), and `results.json`'s `build` field is a constant
  string, so it never caught it.
- Extraction improved independently of the build: ×1.4 → ×2.3–2.5
  (Test 3), ×6.6/×1.0 → ×7.4/×1.7 across the density sweep (Test 4),
  from the flat `findall` path landing after the old run.
- GIL threading now looks completely different from the old text, and
  the page was rewritten accordingly: rurex' pile drops 3.9× from one
  worker to eight (GIL-released scans on four physical cores), so the
  lead is ×31 at one worker and ×231 at eight, where the old edition
  reported a flat 0.5–0.8 s. Test 8 likewise: rurex gains 2.2–3.9×
  from four workers and leads ×69–97.
- The nine result tables had no GFM delimiter row, so the HTML twin
  rendered them as raw pipe text (only the environment table was a
  real table). Delimiters added; the page now has ten tables.
- Free-threaded cost is now measured and stated: the same pile takes
  0.88 s on 3.14t against 0.32 s on GIL Python, i.e. per-object locking
  makes rurex's Python side ~2.7× dearer, while `re` moves only 10.0 →
  11.4 s. That is the next free-threaded target, not the scan.

The two corpus pages were left as published: they ran at `83b6507`
and measure `findall`, which neither of today's commits touched.


### Spec/ADR amendments

- [acted] `spec/docs/architecture.md` "Matching path" now records that
  span conversion is in place with a stack buffer and that compact ASCII
  skips it via `utf8::is_ascii_obj`.
- [acted] No ADR: the change is a pure result-path optimization inside
  the existing safety and API contracts.

### Future-task notes

- [open] The remaining text `search` gap to `re` is one engine-side
  allocation (`Regex::captures_at` builds a `Captures` per call) plus the
  final span vector. The known fix is the `CaptureLocations` reuse listed
  in `report-1792325206-findall-prefilter-speedup.md`, which now also
  covers `search`/`match`/`finditer`, not just grouped `findall`.
- [open] Inline the usual 0-to-3 group spans inside `Match` (small array
  plus heap fallback) to remove the last result-path allocation; measure
  against the ~14% instruction gap to `re` before committing to it.
- [open] `FindIter::step`, `SreScanner::next` and `Scanner::scan` obtain
  the haystack with PyO3's `extract::<&str>`, i.e. CPython's encoder on
  a cold non-ASCII string, instead of the project's AVX2 transcoder.
  Warm-cache steps measure fine (67 µs for the first match of a 160k
  character document, encoder excluded), so the gain is limited to the
  first step of a cold document; it also populates CPython's UTF-8
  cache, which may be a deliberate memory/CPU trade-off nobody has
  written down. Measure before changing it.
- [open] Short non-ASCII strings pay ~570 instructions of fixed cost plus
  ~8 instructions per character to transcode (measured by length sweep).
  On the Telegram `str` row that fixed cost is most of the remaining
  ~90 ns/post gap to `re`, which never encodes. Candidate: fold the
  required-byte prefilter scan and the transcode's first chunk, or cut
  the scratch-pool TLS round trip.
- [open] The shared `regex` cache-pool contention under free-threaded
  Python remains unaddressed; upstream (`regex-automata` docs,
  rust-lang/regex#934) prescribes a per-thread `Regex` clone or an
  explicit per-thread `Cache`, neither of which is in this build.
- [open] The module-level pattern cache still takes a process-wide
  `Mutex`; a module-level `findall` measures 574 ns against 369 ns for a
  compiled pattern on the same input, and the mutex is a convoy under
  free-threading.

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
- [open] `benchmarks/run.py:226` hard-codes the `build` string in
  `results.json`, so a run cannot prove which binary it measured. Record
  something falsifiable instead (`rurex.__file__` plus its mtime or
  sha256, or the cargo profile from the wheel metadata).
- [open] `benchmarks/run.py` ignores unknown flags: `--help` starts a
  full 45-second benchmark run and leaves `/tmp/core4.lock` metadata
  behind (harmless, but confusing). Parse the known flags and reject the
  rest.
- [open] The generated HTML twins are gitignored, so the published page
  is whatever the last local `make_charts.py` wrote; `docs/benchmarks.md`
  and the charts are the tracked truth. Keep the two in sync in the same
  commit.
