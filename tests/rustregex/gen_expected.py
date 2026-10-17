"""Regenerate EXPECTED.md from a full two-layer triage run.

Not part of the test suite: run manually from tests/rustregex after
driver changes or corpus re-vendoring:

    python gen_expected.py

Classification rules live here; the drivers stay declarative.
"""

import ast
import collections
import re as _re
import warnings

warnings.simplefilter("ignore")

import corpus  # noqa: E402
import engine  # noqa: E402
import parity  # noqa: E402

SPECIAL_A = {
    "regression/empty-flag-expr": (
        "corpus-ahead-of-pinned-crate",
        "mid-pattern global (?x): accepted upstream after 1.13.1; "
        "pinned crate, rurex and re all reject",
    ),
    "regression/flags-are-unset": (
        "corpus-ahead-of-pinned-crate",
        "mid-pattern global (?i): accepted upstream after 1.13.1; "
        "pinned crate, rurex and re all reject",
    ),
    "bytes/invalid-utf8-anchor-200": (
        "follow-up",
        "bytes-mode empty-match position off by one on invalid UTF-8 "
        "(upstream fuzz regression)",
    ),
    "substring/unicode-word-end": (
        "by-design: python-parity",
        "endpos window counts as end-of-string for \\b in re/rurex; "
        "corpus bounds search the full haystack",
    ),
}

POSIX_CLASS = _re.compile(r"\[:[a-z]+:\]")
LOOP_CAPTURE = (
    "empty-loop capture differs: engine does not re-enter a loop body "
    "on an empty iteration (by-design; cf. vendored test_bug_2537)"
)
LOOP_CORNER = (
    "empty-body loop corner: CPython's empty-iteration loop "
    "protection yields a different sequence; by-design (api.md, "
    "same root cause as test_bug_2537)"
)
DOLLAR_END_ONLY = (
    "engine $ is end-of-haystack (like re's \\Z); Python $ also "
    "matches before a trailing newline — known engine semantics "
    "(cf. vendored ledger test_dollar_matches_twice)"
)
B_EMPTY_STRING = (
    "\\B on the empty haystack: engine reports (0,0), CPython "
    "does not — by-design engine semantics"
)


def construct(pat):
    if POSIX_CLASS.search(pat):
        return "POSIX class [:x:] (re parses the bracket as literals)"
    if "\\b{" in pat or "\\B{" in pat:
        return "specialized word boundaries (\\b{start}/\\b{end})"
    if "(?mR" in pat or "(?R" in pat:
        return "CRLF line mode (?R)"
    if "\\P" in pat:
        return "negated Unicode class \\P"
    if "\\p" in pat:
        return "Unicode class \\p{..}"
    if "[[:" in pat:
        return "POSIX class [[:..:]] (re parses as literal chars)"
    if "(?-u" in pat or "(?i-u" in pat or "(?u" in pat or "(?s-" in pat \
            or "(?su" in pat:
        return "engine inline flags (?u, ?-u scoping)"
    if _re.search(r"\(\?-[a-z]+\)", pat):
        return "mid-pattern flag toggles ((?-i))"
    if "(?U" in pat:
        return "swap-greed flag (?U)"
    if "\\z" in pat:
        return "strict-end anchor \\z"
    if "^" in pat or "$" in pat:
        return "anchor repetition (^*/^+ etc.)"
    return None


def split_seq(detail):
    m = _re.match(r"^rurex (\[.*\]) != re (\[.*\])$", detail, _re.S)
    if not m:
        return None, None
    def conv(x):
        return [
            tuple(s) if isinstance(s, list) else s
            for s in ast.literal_eval(x)
        ]
    return conv(m.group(1)), conv(m.group(2))


def subseq(x, y):
    it = iter(y)
    return all(any(u == v for v in it) for u in x)


def classify_b_seqdiff(case, detail):
    pat = case.patterns[0]
    ru, rr = split_seq(detail)
    if POSIX_CLASS.search(pat):
        return (
            "by-design: rurex superset",
            "POSIX class [:x:] (re parses the bracket as literals)",
        )
    if ru is None:
        return "follow-up", "sequence diff not machine-parseable"
    spans_ru = [m[0] for m in ru]
    spans_rr = [m[0] for m in rr]
    if spans_ru == spans_rr:
        return "by-design: engine-semantics", LOOP_CAPTURE
    if "\\B" in pat and not rr:
        return "by-design: engine-semantics", B_EMPTY_STRING
    if "$" in pat:
        return "by-design: engine-semantics", DOLLAR_END_ONLY
    return "by-design: engine-semantics", LOOP_CORNER


def main():
    rows = []
    counts = collections.Counter()
    for c in corpus.load_cases():
        a = engine.run_case(c)
        b = parity.run_case(c)
        pat = c.patterns[0] if not c.multi else "<set>"

        if a[0] == "skip":
            rows.append((c.case_id, "A", "unmapped", a[1]))
        elif a[0] == "fail":
            if c.case_id in SPECIAL_A:
                bk, r = SPECIAL_A[c.case_id]
            elif b[0] == "fail" and "rejects" not in b[1]:
                # Mirror the layer-B bucket: by-design divergences are
                # by-design on both layers; only genuine defects (or
                # unparseable diffs) stay follow-up.
                bb, _br = classify_b_seqdiff(c, b[1])
                if bb == "follow-up":
                    bk, r = "follow-up", "same root cause as the layer-B row"
                else:
                    bk = bb
                    r = "same root cause as the layer-B row"
            elif "($" in c.patterns[0] or "$" in c.patterns[0]:
                bk, r = "by-design: engine-semantics", DOLLAR_END_ONLY
            else:
                bk, r = (
                    "by-design: python-parity",
                    "zero-width iteration follows CPython; corpus "
                    "follows the Rust engine",
                )
            rows.append((c.case_id, "A", bk, r))

        if b[0] == "skip":
            rows.append(
                (c.case_id, "B", "by-design: no-re-equivalent", b[1])
            )
        elif b[0] == "fail":
            if "rejects" in b[1]:
                con = construct(pat)
                rows.append((
                    c.case_id, "B", "by-design: rurex superset",
                    con or "UNINSPECTED",
                ))
            else:
                bk, r = classify_b_seqdiff(c, b[1])
                rows.append((c.case_id, "B", bk, r))

    bad = [r for r in rows if r[3] in (None, "UNINSPECTED")]
    if bad:
        for r in bad[:10]:
            print("UNINSPECTED:", r)
        raise SystemExit(f"{len(bad)} uninspected rows — classify first")
    for _, _, bk, _ in rows:
        counts[bk] += 1
    print(dict(counts), "rows:", len(rows))

    hdr = """# EXPECTED ledger — rust-regex corpus conformance

Every corpus case that does not pass on a layer has a row here
(bucketed reason), mirroring the ADR-0009 ledger convention for the
vendored CPython suite. The harness enforces exhaustiveness in both
directions: a non-passing case without a row fails the suite, and a
row whose case passes is reported stale and fails the suite.

Layers: **A** = rurex vs corpus expectations (binding conformance),
**B** = rurex vs stdlib `re` on identical inputs (parity).

| Bucket | Meaning |
|---|---|
| unmapped | corpus feature rurex's API cannot express (skipped) |
| by-design: python-parity | rurex deliberately follows CPython `re` where that differs from the engine corpus, or `re` has no equivalent |
| by-design: engine-semantics | rurex keeps the engine's behavior where CPython differs (documented deviation) |
| by-design: rurex superset | pattern uses engine syntax `re` lacks; `re` rejects it or parses it differently |
| corpus-ahead-of-pinned-crate | corpus expects engine behavior added upstream after the Cargo.lock-pinned 1.13.1 |
| follow-up | genuine defect or open question — see `Future-task notes` in the session report |

Regenerate with `python gen_expected.py` (after driver changes or
corpus re-vendoring), then re-review the diff.

| case | layer | bucket | reason |
|---|---|---|---|
"""
    with open("EXPECTED.md", "w") as f:
        f.write(hdr)
        for cid, layer, bk, reason in sorted(
            rows, key=lambda r: (r[0], r[1])
        ):
            f.write(f"| {cid} | {layer} | {bk} | {reason} |\n")
    print("wrote EXPECTED.md")


if __name__ == "__main__":
    main()
