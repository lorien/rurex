# Single-parse compilation and lazy prefilter derivation

Date: 2026-10-20
Status: accepted

## Context

`Regex::build` used to parse the pattern with `regex-syntax` before
handing the text to the `regex` crate, so every accepted pattern was
parsed twice: once for `re`-shaped error positions, once by the engine
that has to parse it anyway. A required-first-byte prefilter (ADR-0013)
added a third parse, also of the whole pattern.

rebar's `compile` model priced it: rurex was 1.4–2.5× the engine's
compile time and sometimes slower than stdlib `re`
(`03-date/compile-unicode` 8.10 ms against `rust/regex` 5.71 ms and
`re` 3.71 ms, `12-dictionary/compile-single` 16.37 against 8.62 ms).
Compile cost is what dynamically built patterns and short-lived
processes pay; a long scan amortizes it, so the loss was invisible in
the `search`/`count` numbers.

Two facts bound the possible fixes. `regex::Regex` has no HIR entry
point, so the engine's own parse cannot be skipped while keeping the
`regex` crate as the matching engine. And the engine's parse *is* the
same work as the `regex-syntax` pass — its builder runs that parser
with the same options — so reusing a HIR through
`regex_automata::meta::Regex::build_from_hir` would trade one parse for
one parse and buy nothing at compile time, while replacing the whole
matching layer.

## Decision

1. **Compile first; validate only on rejection.** The cheap textual
   passes that enforce `re`'s own rejections (inline-flag folding,
   scoped `(?a:` rewriting, group-name rules, stacked quantifiers) stay
   unconditional and ahead of the engine build. The `regex-syntax`
   pass that produces positional errors runs only when the engine
   rejects the pattern; if it accepts, the rejection was an engine limit
   and keeps the position-less message. Acceptance cannot widen: the
   validator and the engine share one `regex-syntax` version and one
   builder configuration, so any pattern the engine accepts was always
   accepted by the validator.
2. **Derive the first-byte prefilter on the first cold transcode.**
   The prefix-literal extraction needs a HIR, i.e. a parse, and the
   prefilter is consulted only when a cold non-ASCII `str` is about to
   be transcoded. `Regex::build` therefore no longer parses for it: the
   pattern keeps an empty `OnceLock<Option<Prefilter>>`, and
   `utf8::utf8_of_filtered` resolves it through a borrowed
   `prefilter::Cold` only inside the transcode branch. A pattern used
   only on ASCII or warm-cache strings never pays for it at all.
3. **Drop the compile-time nullability probe.** It existed to keep the
   zero-width must-advance retry engine unbuilt for patterns with a
   positive minimum length, but that engine is already built on the
   first empty match, which such a pattern cannot produce. Its
   `regex-syntax` parse was the third parse.
4. **Keep the compile-time engine configuration on the pattern.** The
   lazily built variants (anchored `fullmatch` engine, retry engine,
   prefilter) reuse the stored `flags::Config` instead of re-running
   flag translation.

Net effect: one parse on the path that compiles, which is the engine's
own.

## Consequences

- Compile time now tracks `rust/regex`'s rather than exceeding it
  (`03-date/compile-unicode` 5.71 ms, `12-dictionary/compile-single`
  8.7 ms, `04-ruff-noqa/compile-real` 62 µs on the benchmark host).
  It still trails stdlib `re` where `re`'s parser is simply cheaper;
  the remaining difference is the engine's program compilation, not
  binding overhead.
- The first cold transcode of a pattern pays one `regex-syntax` parse.
  It is bounded by the pattern's size, happens once per pattern, and
  is amortized against the transcode it can skip.
- A rejected pattern now costs one extra parse (the engine's, before
  the validator's). Errors are the rare path.
- Errors, match results and public API are unchanged; the corpus,
  divergence and CPython conformance suites decide that, and they
  pass unchanged.

## Alternatives rejected

- **`regex_automata::meta::Regex::build_from_hir`.** One parse either
  way (the engine's parse is the same parser and options), so no
  compile-time gain, at the cost of rewriting every matching path off
  `regex::Regex`.
- **Eager prefilter + validate-on-rejection.** Leaves two parses; this
  was the smaller half of the win (`03-date/compile-unicode` 6.9 ms
  instead of 5.7 ms).
- **Keeping the prefilter eager but caching the HIR.** The HIR is the
  engine's private output; there is nowhere to put it without
  replacing the engine.
- **A public `compile` fast path that skips the textual checks.** They
  encode `re` rejections the engine does not make (stacked quantifiers,
  group-name rules, mid-pattern global flags); correctness, not
  performance, is the reason they exist.
