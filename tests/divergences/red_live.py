"""Mode L driver — live parity: rurex vs stdlib `re` on the vendored
regex-engine-divergences constructs, using exactly the operation the
corpus encodes (finditer; i/m/s/x flags; everything else ignored).

This is a hard comparison: a mismatch is a signal (bug or a ledgered
by-design family), per the ADR-0011 discipline.
"""

from __future__ import annotations

import re as stdre

import rurex

from red_corpus import flag_bits


def _run(module, construct):
    """-> {"matches": [(text, index), ...], "error": bool}."""
    try:
        rx = module.compile(construct.pattern, flag_bits(construct.flags))
    except Exception:
        return {"matches": [], "error": True}
    try:
        matches = [
            (m.group(), m.start()) for m in rx.finditer(construct.test_text)
        ]
    except Exception:
        return {"matches": [], "error": True}
    return {"matches": matches, "error": False}


def run_case(construct):
    """-> (status, detail). status in {"pass", "fail", "skip"}."""
    ru = _run(rurex, construct)
    ref = _run(stdre, construct)
    if ru == ref:
        return "pass", None
    return (
        "fail",
        f"rurex {ru['matches']!r} err={ru['error']} != "
        f"re {ref['matches']!r} err={ref['error']}",
    )
