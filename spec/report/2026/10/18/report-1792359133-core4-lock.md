## Report on task: `/tmp/core4.lock` benchmark lease

Ad-hoc owner request: add a host-local exclusive lease for the first
four physical cores to the Rust and Python benchmark runners, with
owner metadata, a bounded wait, automatic release on crash, and a hard
failure when the lease cannot be acquired.

### Done

- Added `benchmarks/corelock.py`:
  - fixed path `/tmp/core4.lock`, opened `O_RDWR | O_CREAT` with mode
    `0666` and chmod enforced even under a restrictive umask;
  - exclusive non-blocking `fcntl.flock`, with a 600 s default wait and
    `RUREX_CORE_LOCK_TIMEOUT` override;
  - JSON owner metadata: user, PID, host, CPU list, ISO start time,
    command;
  - prints the owner and waits; on timeout prints the owner and exits
    without running;
  - verifies the opened inode still matches the path (guards against
    `/tmp` cleanup replacing the file);
  - releases with `LOCK_UN` and close in a context manager; crash/exit
    releases automatically and stale metadata is overwritten;
  - `RUREX_CORE_LOCK_HELD` lets parent-held leases pass into children
    without deadlocking;
  - a wrapper CLI for non-Python runners:
    `python benchmarks/corelock.py -- cargo run --release ...`.
- Wired the lease into:
  - `benchmarks/telegram.py` and `benchmarks/frontend_html.py`: the
    whole matrix, including stats and all points, is under one lease;
    direct `--point` runs acquire it themselves;
  - `benchmarks/ab.py`: the whole paired matrix, with children inheriting
    the marker;
  - `benchmarks/run.py`: the parent main suite, with the free-threaded
    child inheriting the marker.
- Added the lease description to `spec/docs/dev.md`, the generated
  Telegram/homepage methodology, and `docs/benchmarks.md`.
- Added `tests/test_corelock.py`: metadata/mode, stale metadata,
  busy timeout and owner print, release on exception, inherited marker,
  text-owner compatibility, and the wrapper command.

### Validation

- Local: 7 corelock tests pass; `make check` passes (45 Rust, 302
  pytest, 3,484 conformance); free-threaded pytest passes.
- Local busy test: a holder process printed JSON owner metadata; a
  second run printed the owner and exited on a short timeout.
- Remote `regexus`: synced the uncommitted tree, ran the corelock tests,
  then ran a one-sample homepage A/A. The lock was initially held by an
  existing Rust runner (`./target/release/examples/bench`, text
  metadata); our waiter printed that owner, waited, acquired the lease,
  and wrote JSON metadata with `cpus=[0,2,4,6]`. Lock file mode was
  `0666`.
- The existing `exp_rurex` Rust and Python runners already use the same
  `/tmp/core4.lock` and `flock`; the Python reader treats their text
  metadata as raw owner text. No changes were made in that repository.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` documents the lease, metadata, timeout,
  and wrapper command.
- [acted] Telegram and homepage generated methodology pages now state
  that the whole matrix holds the lease.
- No ADR: benchmark infrastructure only.

### Future-task notes

- [open] The `exp_rurex` Rust runner writes text metadata with a
  hard-coded `cpus=0,2,4,6`; if that repo becomes part of the standard
  benchmark path, its metadata should record the actual `--cpu-list`.
- [open] `benchmarks/run.py` now takes the lease but does not use the
  warm-up/affinity protocol; if the main suite is treated as a
  first-class benchmark, extend the same protocol to it.
- [open] The wrapper covers Rust commands. If a Rust binary needs a
  native lease, it can reuse the same file/flock contract.

### Tooling/process

- The lock was proven against a concurrent Rust benchmark from another
  session: it waited, printed the text owner, and acquired after the
  Rust process released. This is the intended cross-language behavior.
- Parent/child marker handling avoided a self-deadlock: the parent holds
  the exclusive lease and every child skips acquisition.
- A hard kill of a holder leaves the metadata stale but releases the
  flock, as specified.
