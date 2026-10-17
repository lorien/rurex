"""Mode E driver — engine contract: rurex vs the upstream-recorded
Rust `regex` answers.

Per ADR-0011 this is a *discussion* comparison, never an auto-fail:
rurex is expected to behave like the native regex crate *plus rurex's
documented adjustments* (zero-width must-advance, `$` semantics,
superset syntax acceptance). A mismatch is classified into:

- `documented adjustment` — the construct sits in a known adjusted
  family (possessive quantifiers, POSIX/\\p/\\b{} syntax, `$` before
  newline, engine superset re rejects, ...);
- `version drift` — upstream's recorded answers predate our pinned
  regex 1.13.1 (or their runtime differs);
- `discuss` — unknown; listed as a follow-up row.

The recorded answers are data, not an oracle.
"""

from __future__ import annotations

import json as _json

import rurex

from red_corpus import DATA_DIR, flag_bits, load_recorded

_RECORDED = None

_RAW = {
    c["id"]: c
    for c in _json.loads((DATA_DIR / "constructs.json").read_text())
    if "id" in c
}


def _recorded():
    global _RECORDED
    if _RECORDED is None:
        _RECORDED = load_recorded("rust")
    return _RECORDED


def construct_raw_excludes(construct):
    return _RAW.get(construct.cid, {}).get("excludeFlavors", [])


def construct_raw_flavors(construct):
    return _RAW.get(construct.cid, {}).get("flavors")


def rust_applicable(construct) -> bool:
    """Upstream records answers only for flavors a construct applies
    to (`excludeFlavors` / `flavors`); the rest are skips, not
    mismatches."""
    if "rust" in construct_raw_excludes(construct):
        return False
    fl = construct_raw_flavors(construct)
    return fl is None or "rust" in fl


def _to_char_index(text: str, byte_index: int) -> int | None:
    """Upstream's Rust runner reports byte offsets; its Python runner
    (and rurex) report character indices. Normalize to characters —
    None when the offset is not a char boundary (that is drift)."""
    if byte_index > len(text.encode("utf-8")):
        return None
    try:
        prefix = text.encode("utf-8")[:byte_index].decode("utf-8")
    except UnicodeDecodeError:
        return None
    return len(prefix)


def _norm(rec, text: str):
    if rec is None:
        return None
    matches = []
    for m in rec.get("matches", []):
        ci = _to_char_index(text, m["index"])
        if ci is None:
            # Non-boundary or out-of-range: fall back to the recorded
            # value so the mismatch surfaces in the comparison.
            ci = m["index"]
        matches.append((m["match"], ci))
    return {"matches": matches, "error": rec.get("error") is not None}


def _run_rurex(construct):
    try:
        rx = rurex.compile(construct.pattern, flag_bits(construct.flags))
    except Exception:
        return {"matches": [], "error": True}
    try:
        matches = [
            (m.group(), m.start()) for m in rx.finditer(construct.test_text)
        ]
    except Exception:
        return {"matches": [], "error": True}
    return {"matches": matches, "error": False}


def _documented_adjustment(construct) -> str | None:
    p = construct.pattern
    if any(q in p for q in ("?+", "*+", "++")):
        return "possessive quantifiers: engine parses as stacked repetition (ADR-0002)"
    if "[[:" in p or "\\p" in p or "\\P" in p:
        return "POSIX/Unicode class syntax the engine accepts and re lacks"
    if "\\b{" in p or "\\B{" in p:
        return "specialized word boundaries (\\b{start} etc.)"
    if "\\z" in p or "\\Z" in p:
        return "strict-end anchors (\\z, \\Z)"
    if "(?<" in p or "(?=" in p or "(?!" in p or "(?<=" in p or "(?<!" in p:
        return "lookaround: rejected by both engines, verdict/text may differ"
    if "(?(" in p or "\\1" in p or "(?P=" in p:
        return "backreference/conditional syntax: rejected, error shape differs"
    if "$" in p:
        return "$ anchor: engine $ is end-only; re also matches before trailing newline"
    return None


def run_case(construct):
    """-> (status, detail).

    status in {"pass", "skip", "mismatch-adjusted", "mismatch-drift",
    "mismatch-discuss"}.
    """
    if not rust_applicable(construct):
        return "skip", "construct not applicable to the rust flavor"
    rec = _norm(_recorded().get(construct.cid), construct.test_text)
    if rec is None:
        return "skip", "no recorded Rust answer"
    ru = _run_rurex(construct)
    if ru == rec:
        return "pass", None
    detail = (
        f"rurex {ru['matches']!r} err={ru['error']} != "
        f"recorded {rec['matches']!r} err={rec['error']}"
    )
    # Live `re` is the tie-breaker: where rurex matches `re`, the
    # divergence from the recorded engine answer is by design (the
    # binding follows CPython semantics there).
    from red_live import run_case as live_case

    ls, _ = live_case(construct)
    if ls == "pass":
        return (
            "mismatch-adjusted",
            f"rurex follows live re (documented python-parity "
            f"adjustment); {detail}",
        )
    adj = _documented_adjustment(construct)
    if adj:
        return "mismatch-adjusted", f"{adj}; {detail}"
    return "mismatch-discuss", detail
