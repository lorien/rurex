## Report on task: CPython error-message and position parity

### Task (archived from plan.md)

## CPython error-message and position parity

Status: new
Priority: 2

`checkPatternError` asserts exact CPython message text and position;
rurex currently reports the engine's own wording. Map the tested
messages in `validate_pattern`/error mapping: `unterminated
character set`, `nothing to repeat`, `bad escape (end of pattern)`,
`missing ), unterminated subpattern`, `missing >, unterminated
name`, `multiple repeat` (needs rejection of stacked quantifiers),
the unbalanced-parenthesis position, and inline-flag error texts.
Then drop the "error-message parity" ledger entries (the largest
bucket after buffers).

### Done

- `error::cppythonize_ast`/`cppythonize_hir` translate regex-syntax
  errors to CPython wording and positions: unterminated character
  set, nothing to repeat, bad escape (end of pattern), missing ) /
  unterminated subpattern, unbalanced parenthesis, bad character
  range (text reconstructed with an escape-aware class scan),
  incomplete escape \x, bad escape \p, unexpected end of pattern
  for a trailing `(?`, and the `(?P<`/`(?P=`/`(?#`/unknown-extension
  family (bad character in group name, missing group name, missing
  > / missing ), unterminated name, unknown group name, cannot
  refer to an open group, redefinition of group name with group
  numbering, unterminated comment, unknown extension ?Xx with
  CPython's exact word-length quirks).
- New rejections mirroring CPython 3.11+: stacked quantifiers →
  `multiple repeat` (textual scanner; one lazy/possessive modifier
  allowed, so `a*+` still compiles — documented divergence) and
  mid-pattern global inline flags → `global flags not at the start
  of the expression` (verbose-mode whitespace/comments transparent,
  like CPython).
- Engine-lax group names (`(?P<a.>)`) now rejected by a pre-scan
  (escape-aware, char-boundary-safe — an initial version panicked
  on multibyte patterns; caught by the suite, fixed with boundary
  snapping and ASCII-guarded slicing).
- Template `\g<...>` error positions moved to the content start;
  octal-overflow messages include the literal (`\400`), like `re`.
- Conformance: 65 → 70 methods fully passing, ledger 100 → 95; all
  remaining entries are genuinely by-design (lookaround, backrefs,
  conditionals, atomic, frozen-class AttributeError vs TypeError).
  Own suite 123 → 190 (67 differential error-parity tests).
- Marker reconciliation debt cleared: three stale `[open]` items
  from earlier sessions marked `[acted]`.

### Spec/ADR amendments

- api.md: stacked-quantifier and global-inline-flag difference
  entries rewritten to the new behavior; Errors section notes the
  parity rewriting. plan.md task removed (archived above).

### Future-task notes

- [open] `(?P=name)` (backreference) surfaces the engine's raw
  "unrecognized flag" — a friendlier by-design message would be
  nice if anyone ever complains.
- [open] Conditional groups `(?(...))` produce CPython's exact
  messages in `re` but are unsupported here; the 6 ledgered tests
  citing them stay until/unless conditionals ever ship.

### Tooling/process

- Differential testing caught three bugs in the parity layer itself
  (double backslash from slice+format, wrong redefinition position,
  multibyte slicing panic) before commit — the table-driven
  `tests/test_error_parity.py` pattern (pattern → (msg, pos) vs
  stdlib) is cheap and keeps catching.
- CPython's exact behaviors were read from the vendored test
  assertions, never guessed.
