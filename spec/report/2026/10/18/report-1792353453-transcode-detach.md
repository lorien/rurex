## Report on task: GIL-released transcode for large strings

Ad-hoc owner request, following the negative phase-1 `findall`
iterator experiment: plan and implement the transcode-detach task
first. The goal was the homepage benchmark's GIL multi-thread `str`
gap, where the cold transcode still ran before the detached scan.

### Done

- Added `transcode_maybe_detached` in `src/utf8.rs`. For source
  buffers of at least 32 KB it calls `Python::detach` around the
  existing `transcode`; shorter sources stay GIL-held.
- The UCS pointer crosses the detach as a `usize` because raw pointers
  are not `Ungil`. `utf8_of_filtered` holds the `Bound<PyString>` for
  the whole call, and `str` is immutable, so the buffer cannot move or
  be freed while another Python thread runs.
- The transcode still uses the calling thread's `Utf8Scratch` pool;
  only the GIL is released around it.
- ADR-0014 records the threshold, safety argument, and rejected
  alternatives. `architecture.md`, `index.md`, the homepage benchmark
  prose, and the README performance text were updated.
- The two older reports that listed the detach as an open future task
  are marked acted.

### Measurements

Controlled four-thread microbenchmark on a 64 MiB homepage subset,
five repetitions per point, baseline extension built from `HEAD` in a
separate worktree:

| case | baseline | detach | change |
|---|---:|---:|---:|
| GIL 1 str | 0.5242 s | 0.5200 s | about 1% faster |
| GIL 4 str | 0.2482 s | 0.2352 s | about 5% faster |
| GIL 4 bytes | 0.2140 s | 0.2127 s | control, noise |
| free 4 str | 0.2400 s | 0.2421 s | about 1% slower, noise |

Full homepage benchmark, single cold run per point:

| row | before | after |
|---|---:|---:|
| GIL 4 str | 0.805 s | 0.743 s |
| GIL 4 bytes | 0.574 s | 0.588 s |
| GIL 1 str | 1.389 s | 1.476 s |
| free 4 str | 0.942 s | 0.658 s |

The full run is noisy: the bytes control moved in the wrong direction,
and the single-thread row moved against the controlled result. The
controlled threaded microbenchmark is the reliable evidence for the
change: the detach helps the GIL four-worker `str` case by about 5%,
is neutral single-threaded, and does not affect Telegram because its
posts are far below 32 KB.

### Spec/ADR amendments

- [acted] `spec/docs/adr/0014-gil-released-transcode.md` records the
  decision and the pointer-across-detach safety argument.
- [acted] `spec/docs/architecture.md` describes the 32 KB threshold in
  the text-transcoding section and adds the safety note.
- [acted] `spec/docs/index.md` catalogues ADR-0014.
- [acted] `docs/homepage-benchmark.md` and the README now include the
  detach and the refreshed numbers.

### Future-task notes

- [open] The 32 KB threshold is a flat constant. A width-aware or
  loaded-system-aware threshold, or a 16/64 KB A/B, may be better.
- [open] Free-threaded Python already parallelizes the encode;
  detaching is neutral there within noise. If a future run shows a
  real regression, gate the detach to GIL builds.
- [open] AVX-512 and aarch64 NEON transcoder kernels remain from
  ADR-0012.

### Tooling/process

- **Single-run benchmark points are too noisy for a 5% decision.**
  The fresh-process A/B showed no clear win and even moved a bytes
  control row. A controlled multi-repetition thread microbenchmark on
  a fixed subset separated the signal.
- Building the baseline extension in a temporary `git worktree` with
  a separate `CARGO_TARGET_DIR` made the A/B possible without
  disturbing the working tree; the worktree and target dirs were
  removed afterwards.
- The negative phase-1 `find_iter`/`captures_iter` experiment was not
  committed: controlled single-thread scans were slightly slower, so
  the kill criterion applied. Its only measurable gain was a few
  percent on GIL four-worker `str`, which the transcode detach now
  addresses more directly.
