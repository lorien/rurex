## Report on task: Write the user-facing README (and settle licensing)

Ad-hoc work (owner-directed; six-question requirements round, one
HTML A/B comparison of quickstart scope).

### Done

- Requirements settled with the owner: user-first pitch (install-
  deciders as the audience), `pip install` primary with a pending-0.1.0
  status note plus from-source block, headline = linear-time guarantee
  + `re`-compatible API (Unicode/RegexSet/bytes as secondary bullets,
  no speed claims), fully itemized "Differences from `re`" right after
  the pitch, CI badge only, cookbook-style quickstart (variant B,
  chosen over the tight variant A via rendered HTML comparison in
  `var/`), and MIT-only licensing.
- `README.md` rewritten: badge, pitch, differences + beyond-re lists,
  install, five cookbook blocks (matching, iteration/substitution,
  module functions, RegexSet, bytes `(?u:)`) plus the loud-failure
  example, "Why linear time?" (ReDoS framing), compact Development
  section, MIT license line. Every value in the examples was verified
  against the built extension before writing (spans, subn results,
  exact error text).
- `LICENSE` added: MIT, Copyright (c) 2026 lorien.
- Metadata synced: `Cargo.toml` and `pyproject.toml` now
  `license = "MIT"` (was MIT OR Apache-2.0). Single-licensing under
  MIT is compatible with the dual-licensed `regex` dependency (we
  exercise its MIT grant).
- Verification: `maturin develop` rebuilds clean with the new
  metadata; full suite 94/94 green.

### Spec/ADR amendments

- README content supersedes the stub; no spec/ADR semantics touched.
  [acted]

### Future-task notes

- [acted] At 0.1.0 publish time: add the PyPI badge and drop the
  — resolved 2026-10-16: PyPI + version badges added, install-from-
  source note rewritten for the published state.
  "pending first release" note from the Installation section (owned
  by the publish task).

### Tooling/process

- Rendering A/B candidates as standalone HTML in `var/` worked well
  for owner-side comparison of README structure; worth repeating for
  future doc-shape decisions.
