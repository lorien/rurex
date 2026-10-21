# rurex — faster, linear-time regular expressions for Python with a re-compatible API

[![CI](https://github.com/lorien/rurex/actions/workflows/ci.yml/badge.svg)](https://github.com/lorien/rurex/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/rurex.svg)](https://pypi.org/project/rurex/)

Python bindings to the Rust [`regex`](https://crates.io/crates/regex)
crate: linear-time matching — catastrophic backtracking (ReDoS)
is impossible by construction — and faster than `re` on scans (up
to ×13), extraction, and pathological inputs (`re` hangs forever,
rurex answers in ~250 ns); on bytes input it leads there too. The
one regime where `re` keeps an edge is one-shot scans of many short
non-ASCII `str` objects, where per-call transcode and result building
dominate; on bytes input rurex leads there as well. See *Performance:
str vs bytes* below.
The API — names, flags, keyword arguments, `Match` semantics, even
error messages and positions — matches `re`, verified continuously
against CPython's own test suite.

## Use it as `re`

If your patterns stay inside the supported syntax, rurex is a
drop-in.

```python
import rurex as re        # that's it
```

**Not supported — rejected at compile time with a clear error**
(`rurex.PatternError`):

- Lookaround: `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`
- Backreferences: `\1`, `(?P=name)`
- Conditional groups: `(?(1)yes|no)` (an invalid one is reported with
  `re`'s message and position)
- Atomic groups: `(?>...)`
- `\Z` (spelled `\z` here); octal escapes (`\101`); `LOCALE`/`DEBUG`
  flags

**Divergences that compile anyway** (rare, but know them):

- Possessive quantifiers (`a*+`) compile as nested repetition with
  different semantics — grep your codebase for `*+`, `++`, `?+`
  before swapping in rurex
- `$` does not match before a trailing newline (use `\n?$` to
  emulate `re`)
- Capture groups in unbounded loops report the last non-empty
  iteration: `^((x|y)*)*` gives `"xyy"` where `re` gives `""`
- Under `VERBOSE`, Unicode whitespace is ignored too (`re` ignores
  ASCII only)

Compatibility is verified continuously against a vendored, pinned
copy of CPython's `re` test suite plus a differential suite; every
remaining gap falls into one of the categories above.

- **Linear-time guarantee.** Worst-case *O(m·n)* matching — catastrophic
  backtracking is impossible by construction, so rurex is safe on
  untrusted patterns and untrusted input.
- **`re`-compatible API.** `compile`, `search`, `fullmatch`, `findall`,
  `finditer`, `split`, `sub`, module-level shortcuts with a pattern
  cache — same names, same flags (`re.IGNORECASE` works as-is), same
  `Match` semantics, same error style.

## Differences from `re`

Read this before porting:

- **No backreferences, no lookaround.** The engine cannot express them
  in linear time; patterns using them fail to compile with
  `rurex.error`.
- `prefixmatch()` is the Python 3.15 preferred name for `match()`;
  both ship with identical behavior, exactly like Python 3.15.
- The absolute-end anchor is `\z` (`re` spells it `\Z`), and `$`
  matches at the end of the haystack only — not before a trailing
  newline.
- Octal escapes (`\141`) are rejected; use `\xNN`.
- `LOCALE` is not supported (the engine has no locale mode).
- Everything else mirrors `re` down to the details: zero-width-match
  stepping, `findall` returning `''` for unmatched groups,
  `re.error`-style attributes (`msg`, `pattern`, `pos`, `lineno`,
  `colno`).

Beyond `re`:

- **Unicode-first:** `\p{Greek}`-style classes and full case folding
  by default.
- **`RegexSet`**: compile many patterns and scan for all of them in a
  single pass.
- **Bytes with an opt-in:** bytes patterns use ASCII semantics like
  `re`, and a `(?u:...)` group opts a region into UTF-8-aware
  matching.

## Performance: str vs bytes

rurex's engine matches UTF-8 directly. CPython stores `str`
internally as UCS-1/2/4, so a non-ASCII `str` must be viewed as
UTF-8: rurex transcodes it into a reused buffer, or borrows
CPython's UTF-8 cache when another C-API consumer already filled it.
`re` never converts. Three consequences:

- **Scanning many short non-ASCII `str` inputs once**: `re` is faster
  (1M Telegram posts: 0.25 vs 0.19 µs per post single-threaded on GIL,
  0.28 vs 0.19 on free-threaded Python). A required-first-byte
  prefilter skips the transcode for the 81% of posts with no `@` at
  all; the 19% that carry a mention pay one UCS→UTF-8 encode per call,
  and that encode is the entire gap on this row.
- **If your data is already UTF-8 bytes** (files, network,
  JSONL/log streams), use bytes patterns: no conversion happens at
  all and rurex **beats `re`'s bytes mode** (×1.5 on the same
  corpus, 0.12 s vs 0.19 s). This is the recommended shape for
  high-throughput pipelines — the same advice the official RE2
  binding gives.
- **Long documents**: the transcode amortizes, so rurex leads on both
  forms. On 1,127 real frontend HTML pages (256 MiB subset), GIL
  single-threaded: 0.62 s vs `re`'s 3.42 s on `str`, 0.49 s vs
  1.70 s on bytes. Those documents are above the 4 KB GIL-release
  threshold, so rurex also scales with worker threads, and large
  transcodes release the GIL themselves.

Details and thread-scaling numbers:
[docs/telegram-benchmark.md](docs/telegram-benchmark.md) and
[docs/homepage-benchmark.md](docs/homepage-benchmark.md).

## Installation

```
pip install rurex
```

Binary wheels ship for Linux (x86_64 and aarch64, glibc 2.28+) across
CPython 3.10–3.15, plus `abi3` fallback and free-threaded (`3.14t`,
`3.15t`) wheels. Other platforms install from source and need a
[Rust toolchain](https://rustup.rs):

```
git clone https://github.com/lorien/rurex
cd rurex
pip install .
```

## Quickstart

Matching, ported from `re` muscle memory:

```python
import rurex

rx = rurex.compile(r"(?P<year>\d{4})-(\d{2})", rurex.IGNORECASE)

rx.search("released 2024-10-07")        # first match anywhere
rx.prefixmatch("2024-10-07: log")       # anchored at the start (re.match)
rx.match("2024-10-07: log")             # same method, historic name
rx.fullmatch("2024-10-07")              # the whole string

m = rx.search("released 2024-10-07")
m["year"], m.span(1)                    # '2024', (9, 13)
m.group(0, 2), m.groupdict()            # ('2024-10', '10'), {'year': '2024'}
```

Iteration, splitting, substitution:

```python
rurex.findall(r"\d+", "a1b22c333")                   # ['1', '22', '333']
[m.span() for m in rurex.finditer(r"a*", "baac")]    # [(0,0), (1,3), (3,3), (4,4)]

rurex.split(r"[\s,]+", "a b, c")                     # ['a', 'b', 'c']
rurex.sub(r"(\w+) (\w+)", r"\2 \1", "hello world")   # 'world hello'
rurex.subn(r"a", "b", "banana", count=2)             # ('bbnbna', 2)
```

Module functions work like `re`'s, with the same kind of bounded
pattern cache:

```python
rurex.search(r"\d+", "ab123").span()                 # (2, 5)
```

Many patterns, one pass:

```python
patterns = rurex.compile_set([r"\d+", r"[a-z]+"])
patterns.is_match("abc 12")                          # True
patterns.matches("abc 12")                           # [True, True]
```

Bytes: ASCII semantics like `re`, with an opt-in for UTF-8-aware
regions:

```python
data = "café".encode()                               # b'caf\xc3\xa9'
rurex.findall(rb"\w+", data)                         # [b'caf']
rurex.compile(rb"(?u:\w+)").search(data).group()     # b'caf\xc3\xa9'
```

Unsupported constructs fail loudly, not silently:

```
>>> rurex.compile(r"(\w+)\1")     # backreference: not supported
rurex.error: backreferences are not supported at position 5
```

## Why linear time?

`re` is a backtracking engine: for patterns like `(a+)+b` on
adversarial input, matching time grows exponentially — the classic
ReDoS denial-of-service. rurex's engine compiles patterns to finite
automata and guarantees *O(m·n)* worst-case time (m: pattern size,
n: input size). The trade-off is exactly the differences list above:
constructs that cannot be matched in linear time — backreferences and
lookaround — are not offered.

## Development

See [docs/benchmarks.md](docs/benchmarks.md) for benchmark results and
methodology (rurex vs `re`, parameter sweeps measured end to end).

```
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
make build      # release build
make test
make check      # fmt, clippy, cargo test, full conformance suite
```

Design documents live in
[`spec/docs/`](spec/docs/index.md) (the API contract is
[`spec/docs/api.md`](spec/docs/api.md)). CI runs fmt, clippy, and the
full test suite across Python 3.10–3.15 on every push.

## License

MIT — see [LICENSE](LICENSE).
