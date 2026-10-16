## Report on task: Ad-hoc — PatternError as the canonical exception class name

### Done

- The exception pyclass is now named `PatternError` (was `Error`):
  `type(e).__name__ == "PatternError"` and tracebacks read
  `rurex.PatternError: ...`, matching `re` (3.13+), where
  `PatternError` is the class and `error` is the module-level legacy
  alias. The aliasing mechanism was already identical (`rurex.error
  is rurex.PatternError`); only the class's own name was inverted —
  surfaced by an owner question about which half is the alias.
- Rust-side `create_exception!` ident renamed accordingly
  (`error::Error` → `error::PatternError`); three call sites updated;
  api.md error section now states the canonical name.
- Lock-in test: identity, `__name__` parity with stdlib, and the
  raised type's name. 123 own tests, 20 Rust tests, fmt/clippy
  clean, conformance unchanged (65 passed / 100 ledgered).

### Spec/ADR amendments

- api.md Errors section (canonical name + alias relationship).

### Future-task notes

- None opened.

### Tooling/process

- "Which of the two names is the class?" is a question worth asking
  for every alias pair — identity (`a is b`) can hide an inverted
  primary name; checking `__name__` against stdlib catches it.
