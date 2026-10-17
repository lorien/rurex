"""Regenerate EXPECTED.md from a triage run (mode L: rurex vs live
stdlib `re`). Mode E rows (engine contract) are reported in the
summary but only `discuss` rows would need a ledger entry — currently
there are none.

Run manually from tests/divergences after driver changes or corpus
re-vendoring:

    python gen_expected.py
"""

import collections
import re as _re
import warnings

warnings.simplefilter("ignore")

import red_corpus  # noqa: E402
import red_engine  # noqa: E402
import red_live  # noqa: E402

# rurex is a superset: the engine accepts these and `re` does not.
SUPERSET_MARKERS = (
    r"\p", r"\P", r"(?<", r"\z", r"(?-u", r"\b{", r"\B{", "[[:",
    r"(?R)", r"(?&", r"\h", r"\H", r"\V", r"\v", r"\N", r"\X",
    r"~~", r"&&", r"--", r"(?~",
)
# `re` supports these and the engine does not (compile error in rurex).
RE_ONLY_MARKERS = (
    r"(?=", r"(?!", r"(?<=", r"(?<!", r"(?>", r"(?(", r"\1", r"\2",
    r"\3", r"(?P=", r"\Z", r"\Q", r"\K", r"\G", r"(?|", r"(?C",
    r"(?<name>", r"(?P<",
)
SET_SYNTAX = ("&&", "--", "~~", "[[:", r"\<", r"\>")


def classify(c, ru, ref):
    """-> bucket string or None (unclassified: discuss)."""
    p = c.pattern
    if ru["error"] and not ref["error"]:
        for m in RE_ONLY_MARKERS:
            if m in p:
                return f"by-design: re-only syntax ({m.strip(chr(92))})"
        if r"\0" in p:
            return "by-design: re-only syntax (octal escape)"
        if r"\b{" in p or r"\B{" in p:
            return (
                "by-design: unknown \\b{...} extension errors in the "
                "engine; re reads \\b + literal"
            )
        return None
    if ref["error"] and not ru["error"]:
        for m in SUPERSET_MARKERS:
            if m in p:
                return f"by-design: engine superset syntax ({m.strip(chr(92))})"
        if _re.search(r"(?:\^|\$|\\b|\\B)[*+?{]", p):
            return "by-design: engine superset (anchor/boundary repetition)"
        return None
    # Both compiled: divergent parses or semantics.
    if "-[" in p or "[[" in p:
        return (
            "by-design: engine nested-class syntax re parses as literals"
        )
    for m in SET_SYNTAX:
        if m in p:
            return (
                "by-design: engine class-set syntax "
                f"({m}) re parses as literals"
            )
    if "[[:" in p:
        return "by-design: POSIX classes re parses as literals"
    if any(q in p for q in ("?+", "*+", "++")):
        return "by-design: possessive quantifiers (engine: stacked repetition)"
    if "$" in p:
        return "by-design: $ end-only vs before-trailing-newline"
    if r"\b" in p or r"\B" in p:
        return "by-design: word-boundary semantics differ (unicode vs ascii)"
    return None


def main():
    rows = []
    counts = collections.Counter()
    mode_e = collections.Counter()
    discuss = []
    for c in red_corpus.load_constructs():
        s, _ = red_engine.run_case(c)
        mode_e[s] += 1
        ls, detail = red_live.run_case(c)
        if ls == "pass":
            continue
        ru = red_live._run(__import__("rurex"), c)
        bucket = classify(c, ru, red_live._run(__import__("re"), c))
        if bucket is None:
            discuss.append((c.cid, detail))
            rows.append((c.cid, "discuss"))
            counts["discuss"] += 1
        else:
            rows.append((c.cid, bucket))
            counts[bucket] += 1

    print("mode E:", dict(mode_e))
    print("mode L buckets:")
    for k, n in counts.most_common():
        print(f"  {n:4}  {k}")
    print(f"discuss rows: {len(discuss)}")
    for cid, d in discuss[:20]:
        print(f"  {cid}: {d[:160]}")

    hdr = """# EXPECTED ledger — regex-engine-divergences (mode L)

Mode L compares rurex with **live** stdlib `re` on the vendored
constructs (ADR-0011); the harness enforces exhaustiveness in both
directions. Mode E (vs the recorded Rust answers) currently has zero
`discuss` rows and needs no ledger.

| case | bucket |
|---|---|
"""
    with open("EXPECTED.md", "w") as f:
        f.write(hdr)
        for cid, bucket in sorted(rows):
            f.write(f"| {cid} | {bucket} |\n")
    print("wrote EXPECTED.md,", len(rows), "rows")


if __name__ == "__main__":
    main()
