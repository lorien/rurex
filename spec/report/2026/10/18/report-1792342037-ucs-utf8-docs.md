## Report on task: UCS and UTF-8 explainer documents

Ad-hoc task, not taken from `spec/docs/plan.md`.

### Done

- `docs/ucs-explainer.typ` with its compiled `docs/ucs-explainer.pdf`:
  a 14-page A4 explainer. Code points and the three meanings of
  "character"; UCS-2/UCS-4 and why fixed 16-bit storage died; the UTF
  family with the four UTF-8 byte shapes and a four-encoding
  comparison; CPython's PEP 393 kinds, object layout, lone surrogates
  and UTF-8 cache; Rust's `&str` invariant; why the `regex` crate
  compiles classes into byte automata; the rurex bridge (four sources
  of a `&str`, byte-to-char span mapping, the surrogate divergence);
  a byte-by-byte worked example; a Python/Rust comparison table;
  glossary and a five-point recap.
- `docs/ucs-slides.typ` and `docs/ucs-slides.pdf`: a 9-slide 16:9 deck
  making the same argument in presentation form.
- `make docs-pdf` rebuilds both PDFs and fails with an install hint
  when `typst` is missing. `typst` is a standalone binary, not a build
  dependency of the extension, and the committed PDFs mean a reader
  needs nothing installed.
- `spec/docs/dev.md` gains a "Documents" section: what the sources
  are, what the target does, where the tool comes from.
- Follow-up, same session: both documents now link the repository.
  The explainer carries it on the cover, in the running footer on
  pages 2-14, and in section 10, where `src/utf8.rs`,
  `src/utf8/simd.rs`, `src/regex.rs` and the two ADRs are individual
  links to their `blob/main` paths. The deck's cover links it too.
  Verified in the built PDFs, not just in the source: 20 external
  link annotations across 6 distinct URLs in the explainer, 1 in the
  deck.

The technical claims were read out of the repository rather than
recalled: throughput figures from ADR-0012, and the span, `pos` and
`endpos` conversions from `spans_to_py` and `window_bytes` in
`src/regex.rs`. The two points most easily fudged are argued rather
than asserted: that UCS-2 and UTF-16 differ in kind (fixed versus
pair-carrying), and that a UCS automaton is possible but would be a
second engine with worse byte-level behaviour.

### Spec/ADR amendments

- [open] `docs/` has no index. The new PDFs are reachable only by
  guessing names; a `docs/README.md` listing the benchmark pages and
  the two documents, or a line in `spec/docs/overview.md`, would fix
  it.
- [open] `spec/docs/architecture.md`'s "Text transcoding" section
  could link `docs/ucs-explainer.pdf` as background for readers who do
  not already know PEP 393.

### Future-task notes

- [open] The explainer restates measured numbers from ADR-0012 (0.45,
  0.48 and 1.4-3.8 GB/s; 1.46 s to 0.77 s). Any transcoder change must
  re-check them and regenerate the PDF in the same commit, or the
  document starts contradicting the ADR.

### Tooling/process

- [open] `uv tool install typst` cannot work: the PyPI package is the
  `typst-py` library and ships no console script, so uv refuses it and
  rolls back. The working install is the static musl binary from the
  GitHub releases; `dev.md` now says so instead of leaving the failure
  to be rediscovered.
- [needs-decision] The PDFs are committed even though `.gitignore`
  excludes the generated HTML under `docs/`. Rationale: the HTML twins
  are rebuildable snapshots of benchmark runs, while here the PDF is
  the deliverable. If that judgement is wrong, `docs/*.pdf` joins the
  ignore list and `dev.md` drops its "committed" claim.
