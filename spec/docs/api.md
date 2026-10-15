# Python API

The module is `rurex`. Names mirror Python's `re` module where the
engines share semantics; deliberate deviations are listed at the bottom
and recorded in ADR-0004. This document is the contract the
implementation is written against.

## Module attributes and functions

- `rurex.__version__` — package version (from Cargo metadata).
- `rurex.compile(pattern, flags=0) -> Regex` — validate and compile
  once. The pattern's type selects the mode: `str` compiles a Unicode
  regex, `bytes` compiles a bytes regex.
- `rurex.compile_set(patterns, flags=0) -> RegexSet` — compile many
  patterns at once for single-pass membership matching.
- `rurex.escape(text) -> str` — escape all regex metacharacters.
- `rurex.error` — exception raised for pattern errors; subclasses
  `Exception`, exactly like `re.error`.

There are intentionally no module-level match shortcuts
(`re.search(pattern, text)` style) and no pattern cache: compile once
with `rurex.compile` and reuse the object (ADR-0002, ADR-0004).

## Flags

Int constants, combinable with `|`; short aliases as in `re`:

| Constant         | Effect                                                       |
|------------------|--------------------------------------------------------------|
| `IGNORECASE`/`I` | case-insensitive matching                                    |
| `MULTILINE`/`M`  | `^`/`$` match at line boundaries                             |
| `DOTALL`/`S`     | `.` matches newlines                                         |
| `VERBOSE`/`X`    | whitespace and `#` comments ignored in the pattern           |
| `ASCII`/`A`      | non-Unicode mode: `\w` `\d` `\s` `\b` and case folding are ASCII-only |
| `SWAP_GREED`     | greediness swapped: `*` lazy, `*?` greedy (no short alias: `U` collides with `re`'s `UNICODE`) |

Passing `re.LOCALE`, `re.TEMPLATE`, `re.DEBUG`, or any unrecognized bit
raises `ValueError` naming the flag; unsupported flags are never
silently ignored.

## Regex objects

`Regex(pattern, flags=0)` is equivalent to `rurex.compile`.

Attributes: `pattern`, `flags`, `groups` (number of capture groups),
`groupindex` (name -> number mapping).

Methods. The five text methods accept `pos`/`endpos` logical bounds:
`^` and `\A` anchor at `pos`, and no match reports past `endpos`.
They are code-point indices, translated to engine byte offsets
internally.

- `search(text, pos=0, endpos=None) -> Match | None` — first match
  anywhere in the window.
- `prefixmatch(text, pos=0, endpos=None) -> Match | None` — match
  anchored at the start of the window. This is `re.match` under its
  Python 3.15 name; no `match` alias is provided.
- `fullmatch(text, pos=0, endpos=None) -> Match | None` — the whole
  window must match.
- `is_match(text) -> bool` — cheap existence check, no Match built.
- `findall(text, pos=0, endpos=None) -> list` — strings for patterns
  without groups; tuples of group strings when the pattern has
  groups; unmatched groups are `None`.
- `finditer(text, pos=0, endpos=None) -> iterator of Match` — lazy.
- `split(text, maxsplit=0) -> list` — splits on zero-width matches
  per Python 3.7+ `re` semantics.
- `sub(repl, text, count=0) -> str` — `repl` may be a string using
  `re` template syntax (`\1`, `\g<name>`, `\g<0>`) or a callable
  receiving each `Match`.
- `subn(repl, text, count=0) -> (str, int)`.

`Regex` supports `copy`/`deepcopy` and pickling via `__reduce__`
(recompiles from `pattern` + `flags`; a compiled program is not
pickled). `repr(rx)` renders as `rurex.compile('a+')`, with flags when
non-default.

Zero-width matches: iteration and substitution advance one character
past an empty match, matching Python 3.7+ `re` behavior.

## RegexSet objects

`rurex.compile_set(patterns, flags=0)` compiles an iterable of
patterns into a `RegexSet` that scans for all of them in a single
pass; the flags value applies to every pattern. The v0 surface is
membership only (ADR-0004):

- `is_match(text) -> bool` — true if any pattern matches.
- `matches(text) -> list[bool]` — one boolean per pattern, in order.

No offsets, captures, or `pos`/`endpos` in v0.

## Match objects

Attributes: `re` (the Regex), `string` (the original scanned text —
the Python object, not a re-encoded copy), `pos`, `endpos`,
`lastindex`, `lastgroup`.

Methods and accessors:

- `group(*groups)`, `__getitem__(n)` — group by number or name; `0`
  is the whole match. Unmatched groups yield `None`.
- `groups(default=None) -> tuple`.
- `groupdict(default=None) -> dict` — named groups only.
- `start(group=0)`, `end(group=0)`, `span(group=0) -> (start, end)` —
  code-point offsets; unmatched groups yield `-1`.
- `expand(template) -> str` — expand an `re`-style template
  (`\1`, `\g<name>`, `\g<0>`) against this match.

`repr(m)` renders like `<rurex.Match span=(0, 2), match='aa'>`.

## Offsets

All public offsets — spans, `pos`/`endpos`, split points — are
code-point offsets into the Python text, exactly like `re`. The engine
works in UTF-8 byte offsets internally; the conversion happens inside
the Rust extension and never leaks (the rure-python lesson).

## Semantic differences from `re`

- No backreferences and no lookaround: patterns using them fail to
  compile with `rurex.error`.
- `prefixmatch` instead of `match` (Python 3.15's name); no `match`
  alias.
- No module-level convenience functions and no pattern cache.
- `re`'s absolute-end anchor `\Z` is spelled `\z` by the engine;
  `\Z` in a pattern is a compile error. `\A` works as in `re`.
- `$` matches at the end of the haystack only — unlike `re`, not
  before a trailing newline (emulate `re`'s `$` with `\n?$`).
- Unicode is always on for `str` patterns; `\p{...}` Unicode classes
  and `(?P<name>...)` named groups are available.
- `bytes` patterns match bytes and yield `bytes` results.
- The engine guarantees linear-time matching; catastrophic
  backtracking is impossible by construction.
