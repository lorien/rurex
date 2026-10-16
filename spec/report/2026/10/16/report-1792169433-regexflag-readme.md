## Report on task: Ad-hoc — RegexFlag enum + README drop-in section

### Done

- Flags are now a `RegexFlag` `IntFlag` mirroring `re` (3.11+):
  created once at module init via a small exec'd class body with the
  stdlib member values (ASCII/IGNORECASE/LOCALE/UNICODE/MULTILINE/
  DOTALL/VERBOSE/DEBUG), short names (`I`, `M`, `S`, `U`, `X`, `A`,
  `L`) as aliases, and `re`'s module-prefixed repr trick
  (`str(rurex.I)` == `'rurex.IGNORECASE'`, combined renders as
  `rurex.IGNORECASE|rurex.MULTILINE` — copied from CPython's
  `global_flag_repr` approach; `__module__` fixed via the exec
  globals' `__name__`). `LOCALE`/`DEBUG` exist as constants like in
  `re` and still raise `ValueError` on use. `Pattern.flags` stays a
  plain int — verified first that CPython's own getter returns int.
- Rust internals unchanged (`u32` signatures; IntFlag converts both
  ways — stdlib `re` flags into rurex and rurex flags into stdlib,
  both asserted). `RegexFlag` exposed module-level; surface test
  updated (now 29 public names).
- Conformance: `test_enum` un-ledgered and passing natively —
  71 passed / 94 ledgered / 0 failed. Lock-in test `test_regexflag_enum`.
- README: new "Use it as `re`" section at the very top (owner
  request) — the drop-in claim, the explicit no-go list (lookaround,
  backrefs, conditionals, atomic groups, `\Z`, octal escapes,
  LOCALE/DEBUG), and the compile-anyway divergences (possessive
  footgun, `$`/newline, empty-loop captures, VERBOSE whitespace) —
  flags line dropped since RegexFlag landed first. api.md flags
  section updated.
- Marker reconciliation: the two stale `[open]` error-parity notes
  marked `[acted]`. 191 own tests, 20 Rust tests, fmt/clippy clean.

### Spec/ADR amendments

- api.md (RegexFlag section); README "Use it as `re`".

### Future-task notes

- [open] `Pattern.__repr__` still formats flags by hand
  (`rurex.IGNORECASE|...`); could now reuse the enum's repr — cosmetic,
  revisit with the repr-by-decision ledger entry if ever revisited.

### Tooling/process

- Verified CPython's actual mechanism before implementing: `re`'s
  named str comes from a custom `__repr__` + `object.__str__`
  delegation, and `.flags` returns plain int — two facts that shrank
  the change from "convert the API" to "swap the constants".
