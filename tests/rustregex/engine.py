"""Layer A driver: rurex vs the corpus's expected engine results.

Binding conformance — the vendored corpus (PROVENANCE.md) carries the
expected output of the wrapped engine at the pinned version; this
driver checks that rurex's Python-visible results reproduce it.

Every case resolves to ("pass", None), ("fail", detail) or
("skip", reason). Skips are corpus features rurex's API cannot
express; they are recorded in EXPECTED.md with bucket `unmapped`.
"""

from __future__ import annotations

import rurex

from corpus import (
    IGNORECASE, CorpusError, byte_bounds_to_char, char_spans,
    unescape_haystack,
)


class _Skip(Exception):
    """Internal: corpus feature this harness cannot express."""


def _skip(reason):
    raise _Skip(reason)


def _groups_of(match, ngroups):
    """[span0, span-or-None per capture group]."""
    out = [match.span()]
    for i in range(1, ngroups + 1):
        s = match.span(i)
        out.append(None if s == (-1, -1) else s)
    return out


def _expected_spans(case, groups, bytes_mode):
    """Expected spans for one match, in the engine's offset space.

    `groups` is [span0, group...] in byte offsets (`None` for a
    non-participating group); str-mode spans are converted to char
    offsets to match rurex's str-mode Match.
    """
    if groups is None:
        return None
    if bytes_mode:
        return [tuple(s) if s is not None else None for s in groups]
    return char_spans(case.haystack, groups)


def _prefix_equal(got, want):
    """Element-wise equality over the expected prefix (corpus may list
    only span0 even when the regex has capture groups)."""
    return len(want) <= len(got) and all(
        g == w for g, w in zip(got, want)
    )


def run_case(case):
    """-> (status, detail). status in {"pass", "fail", "skip"}."""
    try:
        return _run(case)
    except _Skip as s:
        return "skip", str(s)
    except CorpusError as e:
        return "skip", f"harness: {e}"


def _run(case):
    if case.opt("search-kind") == "earliest":
        _skip("unmapped option: search-kind=earliest")
    if case.opt("line-terminator") is not None:
        _skip("unmapped option: line-terminator")

    bytes_mode = case.unescape
    flags = IGNORECASE if case.opt("case-insensitive") else 0

    # ---- multi-pattern cases: RegexSet membership only (ADR-0004) ----
    if case.multi and (
        case.opt("match-kind") == "leftmost-first"
        or case.opt("search-kind") == "leftmost"
    ):
        # The corpus runs multi-pattern tests as a union with
        # alternation-priority selection (one winning pattern at the
        # leftmost position); rurex's RegexSet.matches reports
        # membership for every pattern (Rust RegexSet semantics).
        _skip(
            "unmapped: corpus set semantics = alternation-priority "
            "selection; rurex reports membership"
        )
    if case.multi:
        if any(
            case.opt(k) is not None for k in ("anchored", "bounds", "match-limit")
        ) or (case.opt("unicode") is not None and not bytes_mode):
            _skip("unmapped: set test with per-search options")
        try:
            if bytes_mode or case.opt("utf8") is False:
                pats = [p.encode("utf-8") for p in case.patterns]
                haystack = unescape_haystack(case.haystack)
            else:
                pats = list(case.patterns)
                haystack = case.haystack
            rs = rurex.compile_set(pats, flags)
        except rurex.error as e:
            if case.opt("compiles", True):
                return "fail", f"compile_set rejected: {e}"
            return "pass", None
        if not case.opt("compiles", True):
            return "fail", "expected compile failure, compiled fine"
        got = list(rs.matches(haystack))
        want = [
            i in {pid for pid, _ in case.expected}
            for i in range(len(case.patterns))
        ]
        if got != want:
            return "fail", f"set membership {got} != {want}"
        return "pass", None

    # ---- single-pattern cases ----
    if case.opt("match-kind") == "all":
        _skip("unmapped: overlapping iteration (match-kind=all)")
    if case.opt("search-kind") == "overlapping":
        _skip("unmapped: overlapping search on a single pattern")
    if case.group == "regex-lite":
        # This file pins the regex-LITE crate's reduced semantics
        # (ASCII \d/\s/\w/\b/\W even with unicode=true), not the full
        # engine rurex wraps.
        _skip("unmapped: regex-lite crate semantics")
    if case.opt("utf8") is False and not bytes_mode:
        _skip("unmapped: utf8=false non-unescape bytes mode")

    pattern = case.patterns[0]
    if bytes_mode:
        # rurex bytes mode is ASCII-class only (re parity), so the
        # corpus's unicode-aware bytes expectations cannot be checked.
        if case.opt("unicode", True):
            _skip("unmapped: unicode bytes mode")
        pattern = pattern.encode("utf-8")
        haystack = unescape_haystack(case.haystack)
    else:
        if case.opt("unicode") is False:
            _skip("unmapped: unicode=false str mode")
        haystack = case.haystack

    try:
        rx = rurex.compile(pattern, flags)
    except rurex.error as e:
        if case.opt("compiles", True):
            return "fail", f"rurex rejected an engine-valid pattern: {e}"
        return "pass", None
    if not case.opt("compiles", True):
        return "fail", "expected compile failure, compiled fine"

    # Corpus `bounds` are byte offsets; rurex takes pos/endpos in the
    # haystack's native offset space.
    pos, endpos = 0, None
    bounds = case.opt("bounds")
    if bounds is not None:
        if bytes_mode:
            pos, endpos = bounds
        else:
            pos, endpos = byte_bounds_to_char(case.haystack, bounds)

    limit = case.opt("match-limit")

    if case.opt("anchored"):
        m = rx.match(haystack, pos, endpos)
        want = case.expected[:1]
        if m is None:
            if want:
                return "fail", f"no anchored match, expected {want[0][1]}"
            return "pass", None
        if not want:
            return "fail", f"anchored match {m.span()}, expected none"
        want_spans = _expected_spans(case, want[0][1], bytes_mode)
        got_spans = _groups_of(m, rx.groups)
        if not _prefix_equal(got_spans, want_spans):
            return "fail", f"anchored {got_spans} != {want_spans}"
        return "pass", None

    got = [
        _groups_of(m, rx.groups)
        for m in rx.finditer(haystack, pos, endpos)
    ]
    want = [
        _expected_spans(case, groups, bytes_mode)
        for _, groups in case.expected
    ]
    if limit is not None:
        got, want = got[:limit], want[:limit]
    if len(got) != len(want) or not all(
        _prefix_equal(g, w) for g, w in zip(got, want)
    ):
        return "fail", f"matches {got} != {want}"
    return "pass", None
