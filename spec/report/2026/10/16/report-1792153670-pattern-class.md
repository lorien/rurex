## Report on task: Ad-hoc — `Pattern` class-name and `re.Pattern` semantics

### Done

- Class renamed Python-side to `Pattern` (`<class 'rurex.Pattern'>`);
  no `Regex` in the public surface (owner decision; ADR-0008). The
  Rust struct stays `Regex`.
- `re.Pattern` semantics adopted, all verified against CPython 3.13
  first: value `__eq__` (pattern text + flags, str/bytes
  distinguished) with consistent `__hash__` (dict-key usable),
  weakref support (`#[pyclass(weakref)]`), typing subscription
  `Pattern[str]`/`Match[str]` (`__class_getitem__` →
  `PyGenericAlias`), read-only `groupindex` (`mappingproxy`).
- `scanner(text, pos=0, endpos=None)` implemented with
  `SRE_Scanner`'s surface (`.pattern`, `.search()`, `.match()`). The
  differential test exposed a CPython quirk: a **failed `match()`
  consumes the rejected match** — verified against stdlib before
  implementing; parity preserved and pinned by test.
- Bugfix: `match_impl` (shared `match`/`prefixmatch` helper) had been
  exposed as a public Python method by the previous commit (plain fn
  inside `#[pymethods]` is auto-registered by PyO3); moved to the
  plain `impl` block.
- 12 new compat tests (`tests/test_pattern_compat.py`), all
  differential against stdlib where behavior coincides; api.md
  rewritten for the Pattern surface; ADR-0008 + spec index. 107
  Python + 20 Rust tests, fmt/clippy clean.

### Spec/ADR amendments

- ADR-0008 (this commit); api.md Pattern section incl. `scanner`,
  equality/hash/weakref/typing notes, and the documented divergence
  that `rurex.Pattern(...)` is directly constructible while
  `re.Pattern()` is not.

### Future-task notes

- [open] CPython `test_re.py` conformance harness: still blocked on
  `sudo apt install python3-tests` (owner); shim now only needs the
  module-object mapping (`Pattern`/`Match` names, `match` all
  resolve natively).
- [open] `repr(rx)` stays `rurex.compile(...)` by decision (ADR-0008);
  revisit only if the conformance suite proves repr-text assertions
  matter.
- [open] `FindIter` type name (`re` yields a `callable-iterator`);
  low value, revisit with the harness evidence.

### Tooling/process

- Differential testing against stdlib caught a real semantic
  subtlety (scanner match-on-miss consumption) that reading
  docs/source had gotten backwards — when mirroring, encode the
  actual behavior in a test before implementing it.
- PyO3 pitfall recorded: plain fns in `#[pymethods]` blocks become
  public methods; helpers must live in a plain `impl` block.
