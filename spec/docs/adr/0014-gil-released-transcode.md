# GIL-released transcode for large strings

Date: 2026-10-18
Status: accepted

## Context

The engine scans UTF-8, so a cold non-ASCII `str` must be transcoded
before matching. ADR-0012 made that transcode fast, and ADR-0013 skips
it when the required first byte is absent. The remaining `str` cost on
the homepage benchmark is the transcode itself: at four GIL workers,
`str` was 0.81 s against 0.57 s for bytes because the transcode ran
GIL-held before `maybe_detach` released the lock for the scan.

The documents are long (median 117 KB), so the handoff cost is small
relative to the encode. Short strings are the opposite: the Telegram
corpus has a median of about 426 code units, where a handoff costs more
than the transcode.

## Decision

1. **Release the GIL around the transcode when the source is at least
   32 KB.** `transcode_maybe_detached` in `src/utf8.rs` compares the
   source byte size (`len * width`) with the threshold, then calls
   `py.detach` around the existing `transcode` function. Shorter
   sources keep the GIL.
2. **The 32 KB threshold is measured, not derived.** It is above the
   GIL-release threshold for the scan because the transcode is shorter
   than a full scan; a smaller threshold would penalize medium strings.
3. **The pointer remains valid across the detach.** `utf8_of_filtered`
   holds the `Bound<PyString>` for the whole call, `str` is immutable,
   and the UCS buffer therefore cannot move or be freed while another
   Python thread runs. The raw pointer is carried as a `usize` into the
   detached closure because raw pointers are not `Ungil`.
4. **The scratch-pool contract is unchanged.** The transcode still runs
   on the calling OS thread and writes into the same per-thread
   `Utf8Scratch` pool; only the GIL is released around it.

## Consequences

- Homepage GIL four workers: `str` moved from 0.81 s to 0.74 s in the
  full benchmark run; a controlled four-thread microbenchmark on a
  64 MiB subset moved from 0.248 s to 0.235 s (about 5%).
- Single-threaded runs are neutral, within measurement noise.
- Telegram is unchanged: its posts are far below 32 KB and take the
  GIL-held branch.
- Free-threaded Python already runs the encode in parallel; detaching
  is neutral there, within noise.
- The unsafe surface is unchanged in kind: the transcode already read
  the UCS buffer under the validated layout, and the `Bound` keeps it
  alive across the detach.

## Alternatives rejected

- **Always detach:** adds a handoff to every short-string transcode,
  exactly the regime where the handoff dominates.
- **Never detach:** leaves the cold `str` transcode serialized under
  the GIL on long documents, which is the measured gap this change
  addresses.
- **Detach the scan only:** already the behavior; it does not help
  because the transcode runs before the scan.
- **Per-thread transcode workers:** a thread pool for a one-shot encode
  adds scheduling and ownership complexity without helping a single
  document.
