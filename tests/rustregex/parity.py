"""Layer B driver: rurex vs stdlib `re` on identical inputs.

Parity — for every corpus case whose inputs both modules can express,
compile the same pattern with the same flags in both and compare the
Python-visible behavior: compile verdict, and when both compile, the
full match iteration (spans of every capture group, `None` for
non-participants). Expected spans from the corpus are irrelevant
here; a disagreement between the two modules is a bug or a ledger
entry.
"""

from __future__ import annotations

import re as stdre

import rurex

from corpus import IGNORECASE, byte_bounds_to_char, unescape_haystack


def _groups_of(match, ngroups):
    out = [match.span()]
    for i in range(1, ngroups + 1):
        s = match.span(i)
        out.append(None if s == (-1, -1) else s)
    return out


def _seq(rx, haystack, pos, endpos, anchored, ngroups):
    # stdlib patterns default endpos to a huge int, not None.
    if endpos is None:
        endpos = len(haystack)
    if anchored:
        m = rx.match(haystack, pos, endpos)
        return [] if m is None else [_groups_of(m, ngroups)]
    return [
        _groups_of(m, ngroups) for m in rx.finditer(haystack, pos, endpos)
    ]


def run_case(case):
    """-> (status, detail). status in {"pass", "fail", "skip"}."""
    if case.multi:
        return "skip", "re has no RegexSet equivalent"

    bytes_mode = case.unescape
    flags = IGNORECASE if case.opt("case-insensitive") else 0

    if bytes_mode:
        pattern = case.patterns[0].encode("utf-8")
        haystack = unescape_haystack(case.haystack)
    else:
        pattern = case.patterns[0]
        haystack = case.haystack

    # --- compile verdicts must agree ---
    try:
        rrx = rurex.compile(pattern, flags)
        rurex_ok, rurex_err = True, None
    except rurex.error as e:
        rurex_ok, rurex_err = False, e
    try:
        rre = stdre.compile(pattern, flags)
        re_ok, re_err = True, None
    except stdre.error as e:
        re_ok, re_err = False, e
    except Exception as e:  # e.g. FutureWarning-turned-error, deep nesting
        re_ok, re_err = False, e

    if rurex_ok != re_ok:
        who = "rurex" if not rurex_ok else "re"
        err = rurex_err if not rurex_ok else re_err
        return "fail", f"compile verdict differs: {who} rejects ({err})"

    if not rurex_ok:
        return "pass", None  # both reject — agreement

    # --- search behavior must agree ---
    pos, endpos = 0, None
    bounds = case.opt("bounds")
    if bounds is not None:
        if bytes_mode:
            pos, endpos = bounds
        else:
            try:
                pos, endpos = byte_bounds_to_char(case.haystack, bounds)
            except Exception:
                # Off-boundary windows: slice identically for both.
                pos, endpos = bounds

    anchored = bool(case.opt("anchored"))
    try:
        got_rurex = _seq(rrx, haystack, pos, endpos, anchored, rrx.groups)
    except Exception as e:
        return "fail", f"rurex raised {type(e).__name__}: {e}"
    try:
        got_re = _seq(rre, haystack, pos, endpos, anchored, rre.groups)
    except Exception as e:
        return "fail", f"re raised {type(e).__name__}: {e}"

    if got_rurex != got_re:
        return "fail", f"rurex {got_rurex} != re {got_re}"
    return "pass", None
