# Vendored corpus: rust-regex `testdata/`

Source: <https://github.com/rust-lang/regex>, directory `testdata/`,
vendored verbatim.

- Repository: rust-lang/regex
- Tag: `1.13.1` (annotated tag object `7c28ee4dc4aef53571c96757f4825c51ac0c3d44`,
  pointing at commit `2b527599eb9eea0dcc288c704584f242f26a5c61`)
- Vendored: 2026-10-17
- License: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE` here)
- Contents: 1,203 `[[test]]` cases across 23 TOML files + `fowler/`

**Why this exact tag:** `Cargo.lock` pins the `regex` crate this build
of rurex compiles against to 1.13.1; the corpus's expected results are
the ground truth for *that* engine version. When `Cargo.lock`'s regex
version is bumped, re-vendor the corpus at the matching tag and
re-triage the EXPECTED ledger (same rule as the vendored CPython suite,
ADR-0009).

Re-download with:

```sh
curl -sL https://github.com/rust-lang/regex/archive/refs/tags/1.13.1.tar.gz \
  | tar xz --strip-components=1 -C tests/rustregex/data regex-1.13.1/testdata
cp regex-1.13.1/LICENSE-MIT regex-1.13.1/LICENSE-APACHE tests/rustregex/data/
```

The corpus is run by `tests/rustregex/` in two layers (ADR-0010):

- **Layer A — binding conformance:** rurex's Python-visible results
  vs the corpus's expected byte-offset spans.
- **Layer B — parity:** rurex vs stdlib `re` on identical inputs;
  both must produce the same compile verdict and match iteration.

Known by-design failures and unmapped corpus features are listed in
`EXPECTED.md`; the harness enforces ledger exhaustiveness in both
directions.
