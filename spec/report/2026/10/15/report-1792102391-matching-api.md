## Report on task: Implement Match objects and the matching API

### Task (archived from plan.md)

## Implement Match objects and the matching API

Status: done

Implement `src/match_.rs` (Match with `group`/`__getitem__`, `groups`,
`groupdict`, `start`/`end`/`span`, `expand`, `re`, `string`, `pos`,
`endpos`, `lastindex`, `lastgroup`, `__repr__`, `None`/`-1` unmatched
semantics) and the `Regex` methods: `search`, `prefixmatch`,
`fullmatch`, `is_match`, `findall`, `finditer`, `split`, `sub`, `subn`,
with `pos`/`endpos` windows on the five text methods (code-point bounds
translated to byte offsets), a lazy `finditer`, `re`-style templates
plus callable `repl`, copy/deepcopy/pickle via `__reduce__`, and GIL
release around scans per `spec/docs/architecture.md`. `str` and `bytes`
modes both supported.

References: ADR-0002, ADR-0004

### Done

- `src/match_.rs`: frozen `Match` pyclass holding `re`, `string`,
  `pos`/`endpos`, spans pre-converted to Python units, and capture
  names. `group(*args)`/`__getitem__`/`groups`/`groupdict`/
  `start`/`end`/`span` with CPython semantics (`IndexError: no such
  group`, `None`/`-1` for unmatched groups), `lastindex` as the last
  group to close (max end, ties to the lower number — verified against
  CPython cases), `expand`, `__repr__` in `re.Match` format.
- `src/template.rs`: byte-level parser mirroring CPython's
  `parse_template` exactly (two-digit group refs without single-digit
  fallback, three-digit octal escapes, `\g<name|number>`, known
  escapes, octal `\0`, `IndexError` for unknown group names, eager
  validation, CPython-matching positions in error messages).
- `src/regex.rs`: all nine methods. Mechanics: `search` via
  `captures_at` on the endpos-truncated text (preserving real-start
  anchoring); `prefixmatch` = same scan accepted only when the match
  starts at `pos`; `fullmatch` via a lazily compiled `\A(?:pat)\z`
  variant on the window (OnceLock in the frozen class) so `a|aa` on
  `"aa"` succeeds as in `re`, with a span-check fallback if the variant
  cannot build; lazy `FindIter` releasing the GIL per step with
  CPython's zero-width stepping; `split`/`sub`/`subn` on the same
  stepping; `__reduce__` pickle, `__copy__`/`__deepcopy__`,
  `__repr__` with flag names.
- Group text extraction uses Python-level slicing of the stored string
  object (spans in native units), sidestepping PyO3 borrow-lifetime
  friction and keeping `Match.string` the original object.
- Tests: 20 Rust unit tests (templates, offsets, flags); pytest
  `tests/test_match.py` with differential comparison against `re` for
  search/prefixmatch/fullmatch/findall/finditer/split/sub across
  normal, zero-width (`a*` on `baac` spans exactly match CPython's),
  unicode offset, `pos`/`endpos`, bytes-mode, and callable-`repl`
  cases; repr/pickle/copy; error taxonomy. All 53 pytest + Rust
  suites green; fmt/clippy clean.

### Spec/ADR amendments

- api.md: corrected `^`/`\\A` semantics under `pos` (real-text
  anchoring, like `re` — the earlier claim was wrong); findall
  unmatched groups yield `''` not `None`; FindIter documented as the
  lazy iterator; template-error section added; fullmatch-with-`pos`
  corner added to deviations. [acted]
- overview.md layout gained `template.rs`; plan.md task archived
  here. [acted]

### Future-task notes

- [acted] Performance pass later: `Match.names` is cloned per match
  from `FindIter`/`sub`; consider `Rc<Vec<Option<String>>>` shared
  with the Regex. Group extraction via Python slicing could move to
  direct `&str` slicing if PyO3 lifetimes permit a stored borrowed
  text handle.
  — Resolved 2026-10-16: names are now `Arc<Vec<Option<String>>>`
  shared from the Regex (refcount clone per match); `findall` builds
  values by direct `&str`/`&[u8]` slicing, and `piece`/slicing was
  removed from the findall path entirely.
- [open] `escape` for bytes remains str-only (carried over from the
  scaffold report; still unowned by any task).

### Tooling/process

- PyO3 0.30 renames that cost iterations: `allow_threads` is now
  `detach`; `Bound::downcast` is `cast`; `PyString::empty` and
  `PyRef: Clone` do not exist — use `into_pyobject` +
  `to_owned().unbind()` for a self handle; `PyBytes::as_bytes` ties
  its output to the `&Bound` borrow, so haystack extraction uses a
  unified `'py` lifetime through method signatures.
- pymethods with `PyRef` receivers cannot call sibling methods via
  method syntax; use `Self::method(slf, ...)` (moves the PyRef).
- CPython behavior verified empirically and encoded in tests:
  finditer yields empty matches adjacent to non-empty ones;
  `search(r'^b','ab',1)` is None; template errors carry positions
  (bad escape at the backslash, group refs at the first digit).
