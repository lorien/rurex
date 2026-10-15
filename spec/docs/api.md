# Python API

The module is `rurex`. Names mirror Python's `re` module where the engines
share semantics; differences are listed at the bottom. This document is
the contract the implementation is written against.

## Module attributes and functions

- `rurex.__version__` — package version (from Cargo metadata).
- `rurex.compile(pattern, flags=0) -> Regex` — validate and compile once.
- `rurex.escape(text) -> str` — escape all regex metacharacters.
- `rurex.error` — exception raised for pattern errors; subclasses
  `ValueError`.

## Flags

Int constants, combinable with `|`:

| Constant    | Builder option                      |
|-------------|-------------------------------------|
| `IGNORECASE`/`I` | case-insensitive matching        |
| `MULTILINE`/`M`  | `^`/`$` match at line boundaries |
| `DOTALL`/`S`     | `.` matches newlines              |
| `VERBOSE`/`X`    | whitespace and `#` comments ignored in the pattern |

## Regex objects

`Regex(pattern, flags=0)` is equivalent to `rurex.compile`.

Attributes: `pattern`, `flags`, `groups` (number of capture groups),
`groupindex` (name -> number mapping).

Methods (all take `str` or `bytes` text where applicable):

- `search(text) -> Match | None` — first match anywhere in `text`.
- `match(text) -> Match | None` — match anchored at the start of `text`.
- `fullmatch(text) -> Match | None` — the whole `text` must match.
- `is_match(text) -> bool` — cheap existence check, no Match built.
- `findall(text) -> list` — strings for patterns without groups; tuples of
  group strings when the pattern has groups; unmatched groups are `None`.
- `finditer(text) -> iterator of Match`.
- `split(text, maxsplit=0) -> list`.
- `sub(repl, text, count=0) -> str` — `repl` may be a string (with `\1`,
  `\g<name>` backslash substitution) or a callable receiving the `Match`.
- `subn(repl, text, count=0) -> (str, int)`.

## Match objects

Attributes: `re` (the Regex), `string` (the scanned text), `pos`, `endpos`.

Methods and accessors:

- `group(n=0)`, `__getitem__(n)` — group by number or name; `0` is the
  whole match.
- `groups(default=None) -> tuple`.
- `groupdict(default=None) -> dict` — named groups only.
- `start(group=0)`, `end(group=0)`, `span(group=0) -> (start, end)`.
- `lastindex`, `lastgroup`.

## Semantic differences from `re`

- No backreferences and no lookaround: patterns using them fail to compile
  with `rurex.error`.
- Unicode is always on for `str` patterns; `\p{...}` Unicode classes and
  `(?P<name>...)` named groups are available.
- `bytes` patterns match bytes and yield `bytes` results.
- The engine guarantees linear-time matching; catastrophic backtracking is
  impossible by construction.
- Module-level convenience functions (`re.search(pattern, text)` style)
  are intentionally not provided: always compile first. `rurex` has no
  pattern cache.
