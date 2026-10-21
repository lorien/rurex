# Per-thread engine clones for free-threaded scaling

Date: 2026-10-21
Status: accepted (supersedes ADR-0018)

## Context

`regex::Regex` keeps its cache pool outside the `Arc` that holds the
compiled program, so a `Regex` shared across threads serves every thread
but its owner through that pool, and the pool's mutex sits on the search
path. regex-automata documents this exact shape and prescribes cloning
the `Regex` per thread: the clone shares the immutable program and gets
its own pool, so each thread stays on the pool's fast path.

ADR-0018 concluded the opposite — that the pool was not worth
addressing — on the strength of measurements that were invalid. Its
multi-worker probe created a fresh set of worker threads for every
measured round *and* for every case, so by the end of a run the process
had hundreds of live thread states, and the four-worker numbers it
reported (272–400 ns per call, 2.6–3.6× inflation) reflected that
thread churn rather than the engine. Creating the threads once per
process, with one case per process, changes every conclusion.

## Measurements

A throwaway PyO3 probe extension (`/tmp`, not part of the repository)
holding one `regex::bytes::Regex` and exposing the same call two ways —
on the shared engine and on a per-thread clone — over 200k Telegram
posts, threads created once, barrier-started rounds, core lease held:

| probe | 1 worker | 4 workers |
|---|---:|---:|
| shared engine, shared pool | 82 ns | **134 ns** (1.63×) |
| per-thread clone, own pool | 84 ns | **89 ns** (1.06×) |

The same probe also fixes the shape of the call overhead:

| probe | 1 worker | 4 workers |
|---|---:|---:|
| `len(t)` | 39 ns | 32 ns |
| `t.count(b"@")` (C method, same buffers) | 122 ns | 118 ns |
| PyO3 staticmethod, same body | 49 ns | 47 ns |
| PyO3 instance method on a **frozen** class, `(string, pos=0, endpos=None)` | 47 ns | 40 ns |
| PyO3 instance method on a mutable class, same signature | 53 ns | 135 ns |
| the same method with an extra `usize` argument | 69 ns | 246 ns |

Two consequences: a `frozen` pyclass with rurex's exact signature shape
scales perfectly, so the receiver borrow and the optional-argument
defaults are not a problem — but an *extracted integer argument* is
(rurex's defaults are Rust-side, so `pos`/`endpos` are only extracted
when the caller passes them), and a mutable pyclass's borrow flag
costs 2.5× at four workers (rurex's classes are frozen).

In rurex, with the change applied (3 alternating rounds each):

| workload | 1 worker | 4 workers |
|---|---:|---:|
| `is_match`, baseline | 94 ns | 136–152 ns |
| `is_match`, per-thread engines | 96 ns | **99–101 ns** |
| `findall`, baseline | 164–166 ns | 311–346 ns |
| `findall`, per-thread engines | 165–167 ns | **300–308 ns** |
| `finditer`, baseline | 240–245 ns | 683–708 ns |
| `finditer`, per-thread engines | 243–244 ns | 661–687 ns |

Telegram-shaped `findall` over the same posts: 248 → 205 ns per worker
at two workers, 331 → 302 ns at four, unchanged at one.

## Decision

1. **Clone the engine per thread.** A thread-local table keyed by the
   never-reused pattern id from ADR-0017 now holds the engine clone next
   to the `CaptureLocations` scratch, so the per-call cost of finding it
   is the lookup the result path already paid.
2. **Two ids per pattern.** The lazily built anchored `fullmatch` engine
   is a different `Regex`; it takes the pattern id + 1 so a clone of the
   anchored engine can never be handed to the main one.
3. **Every matching site uses the thread's engine**: `is_match`,
   `search`/`match`/`fullmatch`, `finditer` and scanner steps, `sub`,
   `split`, and the flat `findall` scan (one borrow per scan, not per
   match).
4. **Bounded storage.** Four patterns per thread, round-robin eviction;
   each entry holds one engine clone and one location scratch. A clone's
   pool is created empty, so its caches grow only as that thread uses it
   (the engine's own size limits bound them).

## Consequences

- `is_match` at four free-threaded workers: 136–152 → 99–101 ns per
  call, i.e. from 1.5× inflation to 1.04×, at the cost of ~2 ns per call
  single threaded (the thread-local fetch; the same lookup the result
  path already paid for captures).
- `findall` and `finditer` gain less (4% and 3% at four workers) because
  their per-call cost is dominated by the Python result objects they
  create; that is a separate lever, now its own plan record.
- Memory grows with threads × patterns: each thread that matches keeps
  up to four engine clones, each with its own lazily built DFA/hybrid
  caches.
- Correctness is unchanged: a clone shares the immutable program, so
  matches, captures and errors are identical; the full suite passes.

## Alternatives rejected

- **`search_with` and an explicit per-thread `Cache`.** Same effect on
  the pool, but it moves the matching layer onto `regex-automata`'s meta
  API. `regex::Regex` exposes no such entry point; the clone gets the
  same isolation for free.
- **One thread-local engine total (not per pattern).** Thrashing between
  patterns would rebuild a clone and its caches per call.
- **Keying by the engine's address.** Allocators reuse addresses; a
  stale clone of a freed pattern must never be used, and the engine's
  captures API requires the locations of the pattern that made them.
- **Leaving it to the user (advise one compiled pattern per thread).**
  The binding owns the call path; a user cannot opt out of the pool
  without compiling a pattern inside every thread.
