## Report on task: Implement regex compilation and flag mapping

### Task (archived from plan.md)

## Implement regex compilation and flag mapping

Status: done

Implement `rurex.compile` and the `Regex` constructor: flag constants in
`src/flags.rs` translated to `regex::RegexBuilder` options, and
`src/error.rs` mapping `regex::Error` to `rurex.error` with the engine's
message. Unit tests for every flag and for representative bad patterns
(backreference, lookaround, unbalanced paren).

References: ADR-0002

### Done

- Implemented `src/flags.rs`: constants at stdlib `re` bit values
  (I=2, M=8, S=16, U=32, X=64, A=256), `translate()` mirroring
  CPython's `fix_flags` (same messages, `UNICODE` forced on for str
  without `ASCII`, bytes built non-Unicode), plus loud rejection of
  TEMPLATE/DEBUG/LOCALE/unknown bits. SWAP_GREED dropped per the
  revised decision — `(?U)` pattern syntax covers it.
- Implemented `src/regex.rs`: `Regex` frozen pyclass over
  `regex::Regex` / `regex::bytes::Regex` (pattern type selects mode),
  with `pattern` / `flags` / `groups` / `groupindex` attributes.
- Implemented the regex-syntax pre-parse (`regex-syntax` 0.8 direct
  dependency, same flag configuration, `utf8(!bytes_mode)`), giving
  `src/error.rs` full `re.error` attribute parity: `msg`, `pattern`,
  `pos` (char index for str, byte index for bytes), `lineno`,
  `colno`, and CPython-style message formatting. Engine-limit errors
  carry `None` positions.
- `src/lib.rs` now registers `compile`, `Regex`, and the twelve flag
  constant names.
- Tests: 12 Rust unit tests (flag matrix, error messages, byte-to-char
  offsets, line/col math — one real bug caught: `stored_flags` must
  not force `UNICODE` under `ASCII`); 24 pytest tests (attributes,
  normalization parity with `re`, stdlib-flag interop, all rejection
  messages, six bad-pattern attribute cases, non-UTF-8 bytes pattern,
  `(?u:...)` opt-in compile, verbose flag).
- All checks green: `cargo fmt --check`, `clippy -- -D warnings`,
  `cargo test` (12), `maturin develop && pytest` (24).

### Spec/ADR amendments

- ADR-0004 revised (Status line + Revision section): SWAP_GREED
  dropped, re-bit-compatible flags with `fix_flags` semantics, full
  error attribute parity. [acted]
- api.md rewritten: flags table with values, U row, validation
  messages, no-SWAP_GREED note; new Errors and Bytes mode sections
  (`(?u:...)` superset with example); deviations list gains octal
  escapes, non-UTF-8 pattern text, LOCALE unsupported. [acted]
- architecture.md compilation path updated (regex-syntax pre-pass,
  structured errors, bytes non-Unicode default). [acted]

### Future-task notes

- [acted] The matching task (next in plan.md) must consume the stored
  program and deliver `Match` objects with the byte-to-char offset
  conversion already proven in `src/error.rs`.
  (Done in the matching-API task, report-1792102391.)

### Tooling/process

- A crate-root `mod regex` shadows the `regex` crate name at the
  crate root: `regex::escape` in `src/lib.rs` resolved to our module.
  Use `::regex::…` in `src/lib.rs` (submodules are unaffected).
- `regex_syntax::Error` is `#[non_exhaustive]`: matches on
  Parse/Translate need a fallback arm (position-less error).
- Generic helpers over `capture_names()` iterators across both engine
  types fight HRTB lifetimes; two plain match arms are simpler.
- Discovered semantic decisions folded into docs: octal escapes
  unsupported (rejected over silently reinterpreting `\1`-style
  backrefs as octal chars); bytes pattern text must be valid UTF-8.
