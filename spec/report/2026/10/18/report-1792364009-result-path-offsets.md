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
- Collapsed the duplicated per-span conversion loops in `FindIter::step`
  and `SreScanner::next` onto the same helper (`char_at` covers the
  scanner's single anchored cursor), and removed the now-unnecessary
  `chars_seen`/`bytes_seen` counters from `FindIter`. The counters stay in
  `SreScanner`, where `Scanner::scan` uses them to slice tokens.
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
  the haystack with PyO3's `extract::<&str>`, i.e. CPython's 0.45 GB/s
  encoder on a cold non-ASCII string, instead of the project's AVX2
  transcoder. Correct but inconsistent with every other path; it also
  populates CPython's UTF-8 cache, which may be a deliberate memory/CPU
  trade-off that nobody has written down.
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
