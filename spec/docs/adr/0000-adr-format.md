# ADR Format

Date: 2026-10-15
Status: accepted

## Context

The project needs a lightweight, uniform way to record design decisions
that had real alternatives, so later agents can find out *why* the code
looks the way it does without reverse-engineering it.

## Decision

Each record lives in `spec/docs/adr/NNNN-short-title.md`, numbered
sequentially from `0001` (`0000` is this format record). A record
contains:

- a title,
- a `Date:` line,
- a `Status:` line (`accepted`, plus supersession notes such as
  `accepted (superseded by ADR-0007)`),
- a `Context` section (the forces at play),
- a `Decision` section (what was chosen),
- an `Alternatives rejected` section (what was not chosen, and why).

Records are short: the current design lives in the topical documents
under `spec/docs/`; the ADR carries the history and the rationale.
Records are never deleted or renumbered — a later change supersedes by
annotating the old record's `Status:` line. The workflow is defined in
`spec/skills/adr_tracking.md`.

## Alternatives rejected

- Free-form notes scattered in topical docs: decisions lose traceability
  and supersession history.
- A database/tooling-backed log: overkill for a repository of documents
  read by agents and humans.
