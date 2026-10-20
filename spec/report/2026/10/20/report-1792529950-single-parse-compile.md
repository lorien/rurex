## Report on task: Faster pattern compilation: stop parsing twice

### Task (archived from plan.md)

## Faster pattern compilation: stop parsing twice

Status: done

rebar's `compile` model shows rurex at 1.4–2.5× the engine's compile
time and, on some patterns, slower than stdlib `re`:
`03-date/compile-unicode` 8.17 ms against `rust/regex` 5.70 ms and `re`
3.75 ms; `03-date/compile-ascii` 3.62 against 1.46 ms;
`10-bounded-repeat/compile-capitals` 75.7 µs against `re`'s 27.3 µs;
`04-ruff-noqa/compile-real` 92.2 against 62.4 µs.

`Regex::build` parses the pattern with `regex-syntax` to produce
`re`-compatible errors (`pos`, `lineno`, `colno`), then hands the text
to the `regex` crate, which parses and translates it again. The
candidate fix is to compile first and run the `regex-syntax` pass only
when the engine rejects the pattern, keeping the textual checks that
enforce `re` rejections (stacked quantifiers, group-name rules,
mid-pattern global flags) in both paths.

This matters for dynamically built patterns and short-lived processes;
for a long scan it is noise. Parity is the risk: nothing the engine
accepts may become accepted where `re` rejects, so the conformance and
divergence suites decide whether the change ships, and rebar's `compile`
model is the benchmark that proves it.

References: ADR-0002, ADR-0006,
`spec/report/2026/10/19/report-1792446048-rebar-comparison.md`

### Done

- **Compile first, validate on rejection** (`src/regex.rs`): the engine
  builder now runs before the `regex-syntax` pass, which is invoked only
  when the engine rejects the pattern (syntax error → positional
  `re`-shaped error; validator accepts → engine limit → position-less
  error, as before). The textual `re`-only checks (inline-flag folding,
  scoped `(?a:` rewriting, group-name rules, stacked quantifiers) still
  run unconditionally, before the build.
- **Prefilter derived on the first cold transcode** (`src/prefilter.rs`,
  `src/utf8.rs`, `src/regex.rs`): `analyze` → `prefilter::first`, the
  pattern keeps an empty `OnceLock<Option<Prefilter>>`, and the cold
  branch of `utf8_of_filtered` resolves it through a borrowed
  `prefilter::Cold` (slot + pattern text + stored `flags::Config`). ASCII
  and warm-cache strings never resolve it, so the compile path parses
  nothing. `Regex` now stores its `flags::Config`, which the lazily built
  `anchored`/retry/prefilter variants reuse instead of re-running flag
  translation.
- **Dropped the nullability probe**: it existed only to keep the
  zero-width retry engine unbuilt for non-nullable patterns, but that
  engine is already built on the first empty match, which such a pattern
  cannot produce. It was the third parse.
- Verified that nothing new is accepted: `cargo test` (45), `make test`
  (439), the CPython `test_re.py` harness, the rust-regex corpus (both
  layers) and the divergence corpus (both modes) all pass unchanged; a
  direct `re`-versus-rurex check of `error.msg`/`pos`/`lineno`/`colno`
  over the textual-check patterns still matches.
- Re-measured the compile model on `regexus` (see Results) and republished
  the rebar page from a full curated + dictionary run.

### Results

Before/after on the same host and session, `rebar measure -f
'^curated/.*compile' --max-time 3s --max-iters 10`, `/tmp/core4.lock`
lease, pinned to CPU 0, governor `performance`. "Before" is a wheel built
from commit 7834350 (sha256 0a96a7b0…), "after" the wheel built from this
work tree (sha256 fbdc6e0a…).

| benchmark | before | after | stdlib `re` | `rust/regex` |
|---|---:|---:|---:|---:|
| `03-date/compile-ascii` | 3.60 ms | **1.66 ms** | 3.46 ms | 1.48 ms |
| `03-date/compile-unicode` | 8.10 ms | **5.72 ms** | 3.67 ms | 5.65 ms |
| `04-ruff-noqa/compile-real` | 89.51 µs | **68.34 µs** | 69.51 µs | 65.63 µs |
| `05-lexer-veryl/compile-single` | 602.92 µs | **306.51 µs** | 864.65 µs | 281.86 µs |
| `07-unicode-character-data/compile` | 67.70 µs | **34.43 µs** | 101.05 µs | 29.69 µs |
| `09-aws-keys/compile-full` | 161.45 µs | **106.01 µs** | 169.20 µs | 93.42 µs |
| `09-aws-keys/compile-quick` | 28.48 µs | **19.16 µs** | 39.10 µs | 16.20 µs |
| `10-bounded-repeat/compile-capitals` | 75.25 µs | **72.40 µs** | 27.47 µs | 62.79 µs |
| `10-bounded-repeat/compile-context` | 70.10 µs | **58.55 µs** | 45.11 µs | 60.60 µs |
| `11-unstructured-to-json/compile` | 46.71 µs | **25.67 µs** | 75.47 µs | 19.52 µs |
| `12-dictionary/compile-single` | 16.37 ms | **9.01 ms** | 24.95 ms | 8.61 ms |

rurex now tracks the engine's own compile time instead of exceeding it.
Where stdlib `re` still wins, the remainder is the engine's program
build: `03-date/compile-unicode` is 5.79 ms against `re`'s 3.70 ms with
`rust/regex` at 5.71 ms, and `10-bounded-repeat/compile-capitals` is
70.20 µs against `re`'s 27.62 µs with the engine at 61.95 µs — a
short pattern whose compile is almost entirely automata construction.
The µs-scale points move by a few percent between runs; the ms-scale
ones (`03-date`, `12-dictionary`) carry the signal.

Full republished run (curated + dictionary, same protocol): rurex's
geometric-mean score against the fastest engine improves from ×3.04 to
×2.72, with the compile-heavy groups moving most — `12-dictionary`
×1.39 → ×1.03, `11-unstructured-to-json` ×1.82 → ×1.25,
`07-unicode-character-data` ×4.21 → ×2.94, `05-lexer-veryl`
×3.32 → ×2.25, `09-aws-keys` ×4.56 → ×3.77, `03-date` ×6.43 → ×4.82.
Match-bound groups are unchanged within noise (`08-words` ×1.78 → ×1.80,
`06-cloud-flare-redos` ×10.03 → ×10.18), which is the point: the
matching path did not move.

### Spec/ADR amendments

- [acted] New `spec/docs/adr/0015-single-parse-compile.md`: compile-first
  ordering, lazy prefilter derivation, dropping the nullability probe,
  and why `regex_automata::meta::Regex::build_from_hir` buys nothing at
  compile time (one parse either way) at the cost of rewriting the
  matching layer.
- [acted] `spec/docs/adr/0013-cold-str-required-prefix-prefilter.md`
  keeps its decisions and notes the revised timing on its `Status:` line
  ("prefilter derivation timing revised by ADR-0015: computed on the
  first cold transcode, not at compile time"); decision 1 and the
  consequences were adjusted to match.
- [acted] `spec/docs/architecture.md`: the compilation path describes the
  textual checks, the single engine parse and the rejection-only
  validator; the retry-engine bullet now says "built on the first empty
  match" instead of the nullability probe; the transcoding section says
  the prefilter is derived on the first cold transcode; the component
  line says "lazily derived".
- [acted] `spec/docs/api.md`: error positions come from the
  `regex-syntax` parse "when the engine rejects it", not before
  compilation.
- [acted] `spec/docs/index.md` catalogs ADR-0015.
- [acted] `docs/rebar-benchmark.md`/`.html` and the four chart PNGs were
  regenerated from the new run, and the "Compile time" commentary now
  explains the engine's program build instead of the old double parse.
- [acted] `benchmarks/rebar_run.py` grew `--rurex-build`, and
  `benchmarks/rebar_charts.py` renders it in the Method table. The page
  was previously hardcoded to "release wheel from PyPI", which would have
  been false for a wheel built from a work tree; it now names the commit
  and the wheel's sha256.

### Future-task notes

- [open] The remaining gap to `rust/regex` on compile is binding
  constant cost (Python argument handling, `Pattern` construction, the
  capture-name `Arc`, the textual pre-scans). Whether the textual scans
  can be fused into one pass is a separate, smaller change.
- [open] A rejected pattern now pays the engine's parse *and* the
  validator's. Errors are the rare path, but an error-heavy workload
  (a linter compiling user-supplied patterns) would notice; the
  alternative is caching the engine's parse error, which `regex::Error`
  does not expose structurally.
- [open] The lazy prefilter costs one `regex-syntax` parse on the first
  cold transcode of a pattern. For a very large pattern matched once
  against a short cold non-ASCII string that parse can exceed the
  transcode it might skip; a size threshold could defer it further, but
  no benchmark exercises that shape yet.

### Tooling/process

- The benchmark host's non-interactive `ssh` has no `cargo` on `PATH` and
  no default rustup toolchain: export
  `PATH=/home/web/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH`
  before `maturin build` there.
- `/tmp/core4.lock` on `regexus` was unopenable (EACCES, even for root,
  under the host's AppArmor profile) because it was left owned by the
  previous session's user; it could not be removed before the run that
  fixed it. `rm` + recreate made the lease usable again. Worth knowing
  before a measurement session fails at the first line.
- Measuring a work-tree change with rebar needs the local wheel installed
  into `benchmarks/rebar/engines/python/rurex/ve` (`pip install
  --force-reinstall --no-deps`); `rebar build -e` would restore the PyPI
  wheel instead. Both `python/rurex` and `python/rurex-bytes` share that
  venv.
- The compile subset (`-f '^curated/.*compile'`) is a cheap way to see a
  compile-path change (about 10 minutes including warm-up), while the
  full curated + dictionary run is needed to republish the page.
