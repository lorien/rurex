## Report on task: Fix CI (pre-3.12 PEP 393 layout detection)

Owner instruction: "Fix CI." Ad-hoc work, no record archived from
`plan.md`.

### Done

CI had been red since 2026-10-18 on the `python (3.10)` and
`python (3.11)` cells only, with
`tests/test_prefilter.py::test_lone_surrogate_without_the_required_byte_is_no_match`
failing on `UnicodeEncodeError`; 3.12 and newer were green.

Reproduced locally with uv-managed interpreters (`uv python install
3.10 3.11`, venvs under `var/`, `maturin develop --release` into each):
both reproduce the failure exactly. Instrumented `validate()` in
`src/utf8.rs` to print which check rejects the candidate layout, then
dumped the object headers with `ctypes` on 3.10.21, 3.11.16 and 3.13.5.
The pre-3.13 shapes differ from what the code assumed:

| field | 3.12+ | 3.10 / 3.11 |
|---|---:|---:|
| `utf8_length` | `length + 24` | `length + 32` |
| `utf8` | `length + 32` | `length + 40` |
| ASCII data | `length + 24` | `length + 32` |
| non-ASCII data | `length + 40` | `length + 56` |

`from_length` hard-coded `data_ascii = length + 24` for every shape, so
the ASCII-data check failed for the pre-3.12 candidate and detection
returned `None`. The consequence was not just a missing optimization:
with no validated layout, every `str` haystack takes `to_cow`, the
cold-str prefilter never runs, and a surrogate-bearing string reaches
CPython's encoder and raises. That is exactly the test.

Fix: `from_length` takes one `extra` parameter (0 or 8) and applies it
to `utf8_length`/`utf8`/`data_ascii` (+8) and to `data_nonascii` (+16,
because the base object grew by 8 and `wstr_length` adds another 8).
Validation still decides between the candidates, so a future CPython
change keeps degrading to "slow, not wrong". The unit test that
asserted the 3.12 offsets now derives `extra` from the detected layout
and checks the same relations for both shapes.

Checks:

- CPython 3.10.21: 309 pytest, all green (was 1 failed).
- CPython 3.11.16: 309 pytest, all green (was 1 failed).
- CPython 3.13.5: 45 Rust tests, 309 pytest, 3,484 conformance.
- CPython 3.14.8t: 309 pytest.

### Spec/ADR amendments

- [acted] ADR-0012 records that the pre-3.12 shape moves everything
  after `state`, including the ASCII data, and that a single `extra`
  parameter models it; the "ASCII data is always at `length + 24`"
  assumption is called out as the bug that disabled the prefilter on
  3.10/3.11.

### Future-task notes

- [open] `dev.md` should document the throwaway-venv recipe for older
  interpreters (uv install + `VIRTUAL_ENV=… maturin develop
  --release`), because the project venv is 3.13 only and pre-3.12
  regressions are invisible locally.
- [open] CI only sees pushed commits: 19 local commits (all of the
  performance work plus the earlier benchmark republish) had never run
  in CI. Push before trusting a "local tests are green" claim.

### Tooling/process

- A failing pytest cell under an interpreter the developer does not
  have is still reproducible in minutes with uv; the layout
  instrumentation cost one temporary `eprintln!` per failing branch.
- `str.encode()` does not warm CPython's UTF-8 cache, but
  `ctypes.pythonapi.PyUnicode_AsUTF8AndSize` does; the same trick was
  needed to prove the transcode split in the previous session.
