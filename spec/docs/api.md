# Python API

The module is `rurex`. Names mirror Python's `re` module where the
engines share semantics; deliberate deviations are listed at the bottom
and recorded in ADR-0004 (revised). This document is the contract the
implementation is written against.

## Module attributes and functions

- `rurex.__version__` — package version (from Cargo metadata).
- `rurex.compile(pattern, flags=0) -> Pattern` — validate and compile
  once. The pattern's type selects the mode: `str` compiles a Unicode
  regex, `bytes` compiles a bytes regex.
- `rurex.compile_set(patterns, flags=0) -> RegexSet` — compile many
  patterns at once for single-pass membership matching.
- `rurex.escape(text) -> str | bytes` — escape all regex
  metacharacters (ASCII specials and whitespace; other characters
  pass through), mirroring `re.escape` for both types.
- `rurex.error` — exception raised for pattern errors; subclasses
  `Exception`, exactly like `re.error`.

Module-level convenience functions mirror `re`'s, with
`prefixmatch` alongside the historic `match` (both, exactly like
Python 3.15):

- `search(pattern, text, flags=0)`, `prefixmatch(...)`,
  `match(...)`, `fullmatch(...)`, `findall(...)`, `finditer(...)`
- `split(pattern, text, maxsplit=0, flags=0)`
- `sub(pattern, repl, text, count=0, flags=0)`,
  `subn(pattern, repl, text, count=0, flags=0)`
- `purge()` — clear the pattern cache

They are backed by a bounded pattern cache exactly like `re`'s:
keyed by pattern type, text, and flags; at most 512 entries with FIFO
eviction. All error paths are identical to `rurex.compile` since they
share it (ADR-0006; ADR-0002's compile-once advice still applies for
hot loops).

## Flags

Int constants at the stdlib `re` bit values, so `re.IGNORECASE` and
friends can be passed directly:

| Constant          | Value | Effect                                                        |
|-------------------|-------|---------------------------------------------------------------|
| `IGNORECASE`/`I`  | 2     | case-insensitive matching                                     |
| `MULTILINE`/`M`   | 8     | `^`/`$` match at line boundaries                              |
| `DOTALL`/`S`      | 16    | `.` matches newlines                                          |
| `UNICODE`/`U`     | 32    | no-op for `str` (Unicode is the default), error for `bytes`   |
| `VERBOSE`/`X`     | 64    | whitespace and `#` comments ignored in the pattern            |
| `ASCII`/`A`       | 256   | ASCII-only `\w` `\d` `\s` `\b` and case folding (`str` only)  |

Validation mirrors CPython's `re._parser.fix_flags`, including its
messages: `UNICODE` with a `bytes` pattern raises `ValueError: cannot
use UNICODE flag with a bytes pattern`; `ASCII | UNICODE` raises
`ValueError: ASCII and UNICODE flags are incompatible`; `LOCALE` with
a `str` pattern raises CPython's message. Bits the engine cannot honor
are rejected loudly: `LOCALE` with `bytes` (`LOCALE is not supported
by the rurex engine`), `TEMPLATE` (1), `DEBUG` (128), and any unknown
bits (`unknown flag bits: 0x…`). `ASCII` on `bytes` is a no-op, as in
`re`. `Pattern.flags` reports the normalized value like `re` does:
`UNICODE` is forced on for `str` patterns compiled without `ASCII`.

There is no `SWAP_GREED` flag: swapped greediness is pattern syntax,
`(?U)a+?` (ADR-0004, revised).

## Pattern objects

`Pattern(pattern, flags=0)` is equivalent to `rurex.compile`.

Attributes: `pattern`, `flags`, `groups` (number of capture groups),
`groupindex` (name -> number mapping, read-only `mappingproxy` like
`re.Pattern.groupindex`).

Patterns compare by value — equal iff the pattern text (str vs bytes
distinguished) and the flags are equal — with a consistent `__hash__`,
so they work as dict keys exactly like `re.Pattern`. They are
weakref-able and typing-subscriptable (`rurex.Pattern[str]`,
`rurex.Match[str]`), like `re.Pattern`/`re.Match`. Unlike
`re.Pattern` (which cannot be instantiated directly),
`rurex.Pattern(...)` is public and equivalent to `rurex.compile`.

Methods. The five text methods accept `pos`/`endpos` logical bounds.
Exactly like `re`: `pos` only moves where the scan starts (`^`/`\A`
still anchor at the real text start, so `search(r'^b', 'ab', 1)` finds
nothing), and `endpos` behaves as if the text were truncated there.
Both are clamped to the text length first; an `endpos` below the
clamped `pos` yields no match (an empty iterator for `finditer`).
They are code-point indices for `str` and byte indices for `bytes`,
translated to engine byte offsets internally.

- `search(text, pos=0, endpos=None) -> Match | None` — first match
  anywhere in the window.
- `prefixmatch(text, pos=0, endpos=None) -> Match | None` — match
  anchored at the start of the window. The Python 3.15 preferred
  name; `match(...)` is the historic soft-deprecated name for the
  same method, exactly as in `re` (ADR-0007).
- `fullmatch(text, pos=0, endpos=None) -> Match | None` — the whole
  window must match.
- `is_match(text) -> bool` — cheap existence check, no Match built.
- `findall(text, pos=0, endpos=None) -> list` — strings for patterns
  without groups; tuples of group strings when the pattern has
  groups; unmatched groups are the empty string (like `re.findall`;
  `re.split` is the method that yields `None`).
- `finditer(text, pos=0, endpos=None) -> FindIter` — a lazy iterator
  of `Match`; the GIL is released around each scan step.
- `split(text, maxsplit=0) -> list` — splits on zero-width matches
  per Python 3.7+ `re` semantics.
- `sub(repl, text, count=0) -> str` — `repl` may be a string using
  `re` template syntax (`\1`, `\g<name>`, `\g<0>`) or a callable
  receiving each `Match`.
- `subn(repl, text, count=0) -> (str, int)`.
- `scanner(text, pos=0, endpos=None) -> SRE_Scanner` — successive
  `match()`/`search()` over one text, like `re.Pattern.scanner`
  (CPython's `_sre.SRE_Scanner`). The scanner exposes `.pattern`,
  `.search()` (next unanchored match) and `.match()` (next match
  anchored at the cursor).
  `SRE_Scanner` parity quirk preserved: a `match()` miss consumes
  the match it rejected — the cursor advances on every call.

`Pattern` supports `copy`/`deepcopy` and pickling via `__reduce__`
(recompiles from `pattern` + `flags`; a compiled program is not
pickled). `repr(rx)` renders as `rurex.compile('a+')`, with flags when
non-default.

Zero-width matches: iteration and substitution advance one character
past an empty match, matching Python 3.7+ `re` behavior.

## Scanner objects

`rurex.Scanner(lexicon, flags=0)` mirrors `re.Scanner` (ADR-0009):
`lexicon` is a sequence of `(pattern, action)` pairs combined into
one compound pattern `"(p1)|(p2)|..."`. `scan(string)` walks
anchored matches and applies the action of the branch that matched
(`m.lastindex - 1`): callable actions are invoked as
`action(scanner, token)` with the matched text (the last such match
is available as `scanner.match`), non-callable actions are used
as-is, `None` results are dropped, and a zero-width match stops the
scan. `scan` returns `(results, rest)`. Attributes: `scanner` (the
compiled compound `Pattern`), `lexicon`, `match`.

## Bytes mode

`bytes` patterns behave like `re`'s: character classes are ASCII-only
single bytes, and the default has no Unicode matching. The engine
superset remains available inside the pattern — a `(?u:...)` group
opts that region into UTF-8-aware matching, something `re` cannot do:

    rx = rurex.compile(rb"(?u:\w+)")
    # on b"caf\xc3\xa9" this matches the whole word (UTF-8 "café"),
    # while plain rb"\w+" stops at ASCII, like re.

Pattern text must be valid UTF-8 in both modes: a `bytes` pattern
containing raw non-UTF-8 bytes raises `rurex.error` (use `\xNN`
escapes, which under the ASCII default mean the raw byte).

Haystacks and `repl` accept any bytes-like object (`bytearray`,
`memoryview`, `array.array`, `bytes` subclasses), like `re`;
`Match.string` keeps the original object and `group()` returns plain
`bytes`. One-shot methods copy such inputs once per call; `finditer`
and `scanner` copy once at creation — iteration sees a snapshot, so
unlike `re` (which locks the buffer and raises `BufferError` on
resize during iteration), mutation is simply invisible to the
iterator.

## RegexSet objects

`rurex.compile_set(patterns, flags=0)` compiles any iterable of
patterns into a `RegexSet` that scans for all of them in a single
pass; the flags value applies to every pattern. The first pattern's
type sets the mode (`str` or `bytes`); a later pattern of the other
type raises `TypeError: patterns must be all str or all bytes`. An
empty pattern list is allowed and never matches. The v0 surface is
membership only (ADR-0004):

- `is_match(text) -> bool` — true if any pattern matches.
- `matches(text) -> list[bool]` — one boolean per pattern, in order.

No offsets, captures, or `pos`/`endpos` in v0. Text arguments follow
the same mode-mismatch `TypeError`s as `Pattern`, and a bad pattern
raises `rurex.error` with the full attribute set — `pattern` and
`pos` refer to the individual failing pattern.

## Match objects

Attributes: `re` (the Pattern), `string` (the original scanned text —
the Python object, not a re-encoded copy), `pos`, `endpos`,
`lastindex`, `lastgroup`, `regs` (legacy span table: `(start, end)`
per group, `(-1, -1)` for non-participating).

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

## Template errors

`sub`/`subn`/`expand` templates are parsed eagerly (before scanning,
like `re`): a bad template raises even when nothing matches. The
messages mirror `re` (`bad escape \q at position 0`, `invalid group
reference 1 at position 1`, `missing >, unterminated name`). One
CPPython quirk is preserved: an unknown name in `\g<name>` raises
`IndexError: unknown group name 'x'`, not `rurex.error`.

## Errors

`rurex.error` subclasses `Exception` like `re.error` and carries the
same attribute set: `msg` (short reason), `pattern` (the pattern that
failed), `pos`, `lineno`, `colno`. The exception class's canonical name is
`PatternError` (like Python 3.13+); the module-level `error`
attribute is the legacy alias, exactly as in `re`. Positions are
character indices for
`str` patterns and byte indices for `bytes` patterns, exactly as in
`re`. The exception message mirrors `re`'s formatting:
`"{msg} at position {pos}"`, with `(line L, column C)` for multi-line
patterns. Engine-limit errors (compiled program too large) have no
position: `pos`/`lineno`/`colno` are `None` and the message carries the
engine's text.

The structured positions come from parsing the pattern with
`regex-syntax` (the engine's own parser) before compilation; flag
problems are `ValueError` (bad arguments), pattern problems are
`rurex.error` — the same split `re` makes.

## Offsets

All public offsets — spans, `pos`/`endpos`, split points — are
code-point offsets into the Python text, exactly like `re`. The engine
works in UTF-8 byte offsets internally; the conversion happens inside
the Rust extension and never leaks (the rure-python lesson).

## Semantic differences from `re`

- No backreferences and no lookaround: patterns using them fail to
  compile with `rurex.error`.
- `prefixmatch` is the preferred name for `re.match`, which also
  ships as `match` (soft deprecated, like Python 3.15; ADR-0007).
- `fullmatch` with `pos > 0` on patterns containing an explicit
  `^`/`\A` anchors at the window start instead of failing (the
  anchored fallback variant operates on the window); plain
  `fullmatch(text)` is unaffected.
- `re`'s absolute-end anchor `\Z` is spelled `\z` by the engine;
  `\Z` in a pattern is a compile error. `\A` works as in `re`.
- `$` matches at the end of the haystack only — unlike `re`, not
  before a trailing newline (emulate `re`'s `$` with `\n?$`).
- Octal escapes (`\101`) are not supported; the engine rejects them
  (with a backreference hint) rather than silently reinterpreting
  them. Use `\xNN`/`\u{..}`.
- Pattern text must be valid UTF-8, including for `bytes` patterns
  (raw non-UTF-8 bytes in the pattern are rejected; `\xNN` escapes
  produce raw-byte matching under the ASCII default).
- Under `VERBOSE`, the engine ignores Unicode whitespace (e.g.
  `\x85`, `\xa0`); `re` ignores ASCII whitespace only.
- Stacked quantifiers (`a**`, `a*+`) are accepted by the engine as
  nested repetition; `re` rejects them as `multiple repeat` and
  reserves `a*+` for possessive quantifiers (unsupported here).
- Capture groups inside an unbounded loop report the engine's last
  successful iteration; `re` can re-enter a loop body for a final
  empty iteration (`^((x|y)*)*` group 1 is `""` in `re`, `"xyy"`
  here).
- Methods take the `re` keyword names (`string`, `pos`, `endpos`,
  `maxsplit`, `repl`, `count`) so keyword call sites port directly.
- `LOCALE` is unsupported on both pattern types (the engine has no
  locale mode); `re` allows it on bytes.
- Unicode is always on for `str` patterns; `\p{...}` Unicode classes
  and `(?P<name>...)` named groups are available. For `bytes`
  patterns Unicode classes require a `(?u:...)` group (see Bytes
  mode).
- The engine guarantees linear-time matching; catastrophic
  backtracking is impossible by construction.
