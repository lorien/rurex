# Per-thread capture-location scratch instead of `Captures` per call

Date: 2026-10-20
Status: accepted

## Context

`regex::Regex::captures_at` builds a `Captures` and allocates its
location vector on every call. Every `search`, `match`, `fullmatch`,
`finditer` step, scanner step, `sub` step and grouped `findall` match
went through it, so a `search` on a short text paid one allocation and
one free for a result the caller immediately copied out. The result-path
report had already identified it as the largest remaining item in the
text `search` gap to stdlib `re`.

`captures_read_at` does the same search but writes into a
caller-provided `CaptureLocations`, which `Regex::capture_locations()`
builds once per pattern.

Measured on the benchmark host with a 470-character text and 300,000
`search` calls per round, five rounds, leased core, pinned CPU:

| pattern | before | after |
|---|---:|---:|
| no capture groups | 118.5–119.8 ns/call | 99.6–100.6 ns/call |
| one group | 179.0–181.2 ns/call | 162.0–163.4 ns/call |
| two groups | 173.9–176.3 ns/call | 160.6–162.3 ns/call |

## Decision

1. **One `CaptureLocations` per pattern per thread.** The engine's
   `Regex` is shared across threads, so the scratch cannot live in the
   pattern without a lock on every call; it is thread-local instead,
   the same shape as the module cache's per-thread front (ADR-0006).
2. **Keyed by a never-reused pattern id.** A global `AtomicU64` hands
   out ids at compile time. Keying by the pattern's address instead
   would let a freed pattern's slot be picked up by a new one at the
   same address; `captures_read_at` documents that its locations are
   only valid for the pattern that created them, so a stale hit would
   be a correctness bug, not just a miss.
3. **Four slots, round-robin eviction.** Enough that alternating
   between a few patterns does not thrash, small enough that a thread's
   scratch is bounded (four location vectors, each `2 × (groups + 1)`
   words). Ids are never reused, so an evicted slot is simply
   overwritten by the next pattern that misses.
4. **Copy the spans out immediately.** `captures_read_at` fills the
   scratch, `Spans` is filled from it, and the borrow ends; no state
   from the engine outlives the call. The flat `findall` scan, which
   walks many matches in one Rust pass, borrows the scratch once for
   the whole scan.
5. **Bytes mode twice over.** Text and byte patterns need different
   location types, so each mode has its own thread-local table.

## Consequences

- `search` on a short text is 8–16% faster per call, and the same
  saving applies to every `finditer`/scanner/`sub` step and to each
  grouped `findall` match, since they all went through the same
  allocation.
- Re-entrancy is unchanged: the scratch is borrowed only while the
  engine searches, and no Python code runs there. A user callback
  (`sub` with a callable, a `Scanner` action between steps) runs after
  the borrow is dropped.
- Memory: one `u64` per pattern plus, per thread that matches, up to
  four location vectors. Patterns never used on a thread cost nothing
  there.

## Alternatives rejected

- **Keep `captures_at`.** One allocation and free per call on the
  hottest result path.
- **`CaptureLocations` as a `Regex` field.** The pattern is shared
  across threads; a `Mutex` would serialize every search, which is the
  contention ADR-0006's per-thread front exists to avoid, and a
  `RefCell` field is not `Sync`.
- **A single thread-local slot.** Thrashing whenever two patterns are
  used in turn (`finditer` interleaved with another pattern), turning
  every call into a construction cost instead of a saving.
- **A global map from pattern id to locations.** Correct, but pays a
  lock or a sharded map lookup per call for something that is already
  thread-local by nature.
- **Keying by the pattern's pointer.** Cheaper than an id, unsound:
  allocators reuse addresses, and a stale slot with a different group
  count must never be handed to `captures_read_at`.
