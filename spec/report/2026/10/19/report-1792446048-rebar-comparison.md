## Report on task: rurex in rebar (third-party engine comparison)

Owner request: measure rurex with rebar against `python/re` and
`rust/regex`, in both the text and the bytes representation. Ad-hoc
work, no record archived from `plan.md`.

### Done

- BYOB copy of BurntSushi/rebar in `var/rebar` (throwaway; the two
  durable pieces are now tracked under `benchmarks/rebar/`):
  `main.py`, rebar's Python runner adapted to `rurex` with an extra
  `rurex-bytes` mode, and `add_engines.py`, which registers
  `python/rurex` on every benchmark that lists `python/re` (225 entries
  across 46 definition files) and `python/rurex-bytes` on the one
  Unicode benchmark whose haystack is ASCII
  (`dictionary/search/english-15`).
- Two `[[engine]]` blocks in rebar's `benchmarks/engines.toml` install
  the released wheel (`rurex==0.2.0`) into a venv. Our copy had to
  change `python/re`'s dependency probe from `python` to `python3`:
  the host has no `python` binary, so rebar refused to build stdlib `re`
  until that was patched.
- Protocol: rebar built from master on `regexus`, run under
  `/tmp/core4.lock` (`cpus=[0,2,4,6]`) with `taskset -c 0`,
  `--max-time 3s --max-iters 10`, governor `performance`, load ~0.
  Every point verified `OK` with `rebar measure --test` before timing.
- Measured: the curated set, 44 benchmark/engine points shared by the
  three engines (140 points total).
- `benchmarks/rebar/README.md` documents the setup, the engine blocks and
  the invocation; `spec/docs/dev.md` points at it.

### Results

`rebar rank … --intersection` over the curated set (geometric mean of
speed ratios, lower is better):

| engine | version | geometric mean | benchmarks |
|---|---|---:|---:|
| `rust/regex` | 1.12.4 | 1.08 | 44 |
| `python/rurex` | 0.2.0 | 3.04 | 44 |
| `python/re` | 3.13.5 | 9.78 | 44 |

The published page (`docs/rebar-benchmark.md`, generated from the
committed CSVs) is authoritative for the numbers; this report keeps the
stable findings and the decisions.

Per group, ratio against `rust/regex` (my own recomputation from the
medians, so it differs slightly from the weighted ranking above):

| group | rurex | `re` |
|---|---:|---:|
| 01-literal | 2.76 | 9.92 |
| 02-literal-alternate | 1.62 | 21.85 |
| 03-date | 6.39 | 13.42 |
| 04-ruff-noqa | 5.83 | 9.33 |
| 05-lexer-veryl | 3.23 | 3.82 |
| 06-cloud-flare-redos | 10.22 | 814.94 |
| 07-unicode-character-data | 4.22 | 5.03 |
| 08-words | 1.76 | 1.39 |
| 09-aws-keys | 4.45 | 5.00 |
| 10-bounded-repeat | 1.23 | 2.18 |
| 11-unstructured-to-json | 1.77 | 1.76 |
| 12-dictionary | 1.44 | 108.67 |
| 14-quadratic | 1.77 | 4.48 |

Individual points worth reading (median, µs; rurex / `re` / rust-regex):

- `01-literal/sherlock-en` 65.8 / 256.2 / 26.4 — ASCII literal, rurex is
  2.5× off the Rust engine and 3.9× faster than `re`.
- `01-literal/sherlock-ru` 224.9 / 225.0 / 48.7 — the Russian haystack
  is transcoded on every call, so rurex lands exactly on `re` and 4.6×
  behind the Rust engine.
- `08-words/all-russian` 9960 / 3000 / 6110 — the one group where `re`
  beats rurex on average: Unicode `\w`/`\b` plus the transcode.
- `03-date/unicode` 46150 / 333570 / 1980 — a slow pattern for both
  bindings, but rurex is 7× ahead of `re`.
- `12-dictionary/single` 97.3 / 370410 / 89.4 — `re` is four orders of
  magnitude off; rurex is 1.09× the Rust engine.
- `06-cloud-flare-redos/simplified-long` 0.8 / 26530 / 0.1 — the ReDoS
  case: rurex answers linearly, `re` does not.

Both modes on identical input: `dictionary/search/english-15`
(Unicode mode, ASCII haystack, 61 KB) — `python/rurex` 664 MB/s,
`python/rurex-bytes` 678 MB/s, `rust/regex` 687 MB/s, `re`
166 KB/s. The bytes engine is 2.1% faster because the haystack is
ASCII, where the text path borrows instead of transcoding; the 3.3%
between rurex and the Rust engine is the Python boundary.

### Spec/ADR amendments

- [acted] `spec/docs/dev.md` "Benchmarks" points at
  `benchmarks/rebar/README.md` for the third-party comparison.
- [acted] No ADR: benchmark tooling, no library behaviour.

### Future-task notes

- [acted] The `08-words` loss was re-scoped after reading the
  definitions: `all-english` is byte mode with 56,601 matches and is
  per-match Python object cost (63 ns/match against `re`'s 40 and the
  engine's 13), now a plan task; the two Russian points are upstream
  engine behaviour (its DFA cannot handle a Unicode `\b` on non-ASCII
  bytes, so `rust/regex` itself is 2x `re`) and the transcode is under 1%
  of that call, so there is nothing for the binding to fix there. The
  rebar page states both.
- [open] The bytes mode is only sound where both engines agree on the
  match count, i.e. ASCII haystacks in Unicode mode. rebar has exactly
  one such benchmark, so the non-ASCII str-versus-bytes split remains
  measured only by `docs/telegram-benchmark.md`; a BYOB benchmark pair
  (same non-ASCII haystack, Unicode-mode `str` versus bytes) would close
  that gap inside rebar.
- [open] Upstreaming is a PR with the runner, the `engines.toml`
  entries and the definition edits; it also needs the `python`-versus-
  `python3` dependency nit resolved, since upstream's `python/re` entry
  assumes a `python` binary exists.
- [open] `var/rebar` (clone, venvs, build) is 473 MB of throwaway; the
  tracked `benchmarks/rebar/` is the reproducible part.

### Tooling/process

- rebar is a single-threaded, per-point bounded workload: the whole
  curated matrix over four engines took 4m20s on one core, so the core
  lease plus `taskset -c 0` is sufficient and leaves the rest of the
  host alone.
- `rebar build -e` is the cheap way to get only the engines you need;
  building the Python engines costs about 20 s, the Rust runner about
  the same.
