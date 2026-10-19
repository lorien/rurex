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

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` "Benchmarks" now states that published
  runs go to the benchmark host, with the rsync/build/pull-back
  procedure; `spec/docs/testing.md` final checks point at it.
- [acted] `spec/docs/architecture.md` "Matching path" records the
  in-place span conversion and the per-step GIL rule.
- [acted] No ADR: result-path optimizations inside the existing safety
  and API contracts.

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
