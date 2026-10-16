"""Benchmark case definitions.

Each case is a dict with:
- name: short identifier used in tables and charts
- regime: chart/results grouping
- pattern / repl: the inputs
- text: the haystack (deterministic, seeded)
- mode: "str" (Regex methods) or "bytes"
- what: human description for the results page

Compile and module-dispatch cases are defined in run.py (they need
different operation shapes).
"""

import random


def _english(n_bytes, rng, extra=""):
    vocab = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
        "lorem", "ipsum", "dolor", "sit", "amet", "hello", "world",
        "alpha", "beta", "gamma", "delta", "omega", "text", "line",
        "word", "value", "item", "node", "path", "data", "code", "rust",
    ]
    out = []
    size = 0
    while size < n_bytes:
        w = rng.choice(vocab)
        out.append(w)
        size += len(w) + 1
    text = " ".join(out)
    if extra:
        text = text[: n_bytes - len(extra) - 1] + " " + extra
    return text[:n_bytes]


def _numbers(n_bytes):
    rng = random.Random(7)
    chunks = []
    size = 0
    while size < n_bytes:
        n = rng.randrange(100, 10000)
        chunks.append(str(n))
        size += len(chunks[-1]) + 1
    return " ".join(chunks)[:n_bytes]


def _phones(n_bytes):
    rng = random.Random(11)
    chunks = []
    size = 0
    while size < n_bytes:
        chunks.append(f"{rng.randrange(100, 1000)}-{rng.randrange(1000, 10000)} ")
        size += len(chunks[-1])
    return "".join(chunks)[:n_bytes]


def _multilingual(n_bytes):
    blocks = [
        "καλημέρα κόσμε αλφάβητο ",
        "привет мир язык ",
        "汉字测试文本语言 ",
        "café naïve résumé straße ",
        "plain ascii words 123 ",
    ]
    out = []
    size = 0
    i = 0
    while size < n_bytes:
        b = blocks[i % len(blocks)]
        out.append(b)
        size += len(b.encode("utf-8"))
        i += 1
    return "".join(out)[:n_bytes]


rng_scan = random.Random(3)
BIG = 10 * 1024 * 1024
MED = 1024 * 1024

# Scans over large haystacks; the literal targets sit at the very end,
# the worst case for a scanning engine.
SCAN_TEXT_NEEDLE = _english(BIG, rng_scan, extra="the-needle-string")
SCAN_TEXT_RARE = _english(BIG, random.Random(4), extra="zebra4242")
SCAN_TEXT_CLASS = _english(BIG, random.Random(5))
SCAN_TEXT_END = _english(BIG, random.Random(6), extra="xyzzy")

CASES = [
    dict(
        name="is_match-literal-10MB",
        regime="scan",
        method="is_match",
        pattern="the-needle-string",
        text=SCAN_TEXT_NEEDLE,
        what="Literal existence check, 10 MB, match at the end",
    ),
    dict(
        name="search-literal-10MB",
        regime="scan",
        method="search",
        pattern="the-needle-string",
        text=SCAN_TEXT_NEEDLE,
        what="Literal search, 10 MB, match at the end",
    ),
    dict(
        name="search-rare-word-10MB",
        regime="scan",
        method="search",
        pattern=r"zebra\d{4}",
        text=SCAN_TEXT_RARE,
        what="Class+digits pattern, 10 MB, one match near the end",
    ),
    dict(
        name="search-anchor-end-10MB",
        regime="scan",
        method="search",
        pattern=r"xyzzy$",
        text=SCAN_TEXT_END,
        what="Anchored pattern, 10 MB, match at the end",
    ),
    dict(
        name="findall-digits-1MB",
        regime="match-heavy",
        method="findall",
        pattern=r"\d{3,4}",
        text=_numbers(MED),
        what="findall over ~120k matches, 1 MB",
    ),
    dict(
        name="finditer-consume-1MB",
        regime="match-heavy",
        method="finditer",
        pattern=r"\w+",
        text=_english(MED, random.Random(8)),
        what="finditer fully consumed (span() per match), 1 MB",
    ),
    dict(
        name="sub-template-1MB",
        regime="match-heavy",
        method="sub",
        pattern=r"(\d{3})-(\d{4})",
        repl=r"\1.\2",
        text=_phones(MED),
        what="sub with a group template over ~40k matches, 1 MB",
    ),
    dict(
        name="search-short-64B",
        regime="overhead",
        method="search",
        pattern=r"\d+",
        text="no digits at all here, none whatsoever 123",
        what="search on a 55-byte text (call overhead bound)",
    ),
    dict(
        name="findall-unicode-1MB",
        regime="unicode",
        method="findall",
        pattern=r"\w+",
        text=_multilingual(MED),
        what="\\w+ over multilingual (Greek/Cyrillic/CJK/Latin) text",
    ),
    dict(
        name="findall-greek-1MB",
        regime="unicode",
        method="findall",
        pattern=r"[α-ω]+",
        text=_multilingual(MED),
        what="Greek-range class over multilingual text",
    ),
    dict(
        name="findall-bytes-1MB",
        regime="bytes",
        method="findall",
        pattern=rb"\d+",
        text=_numbers(MED).encode(),
        mode="bytes",
        what="bytes-mode findall over digits, 1 MB",
    ),
]

COMPILE_CASES = [
    dict(name="compile-simple", regime="compile", pattern=r"\d{4}-\d{2}-\d{2}",
         what="Simple date pattern"),
    dict(name="compile-url", regime="compile", pattern=r"https?://[a-z0-9.-]+/[\w./-]*",
         what="URL-ish pattern"),
    dict(
        name="compile-datetime",
        regime="compile",
        pattern=r"(?P<year>\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})",
        what="ISO datetime with named and numbered groups",
    ),
]

DISPATCH_CASE = dict(
    name="module-search-cache-hit",
    regime="overhead",
    pattern=r"\d+",
    text="value 123 found" * 4,
    what="Module-level search(), pattern cache hit on both sides",
)

PATHOLOGICAL = dict(
    pattern=r"(x+x+)+y",
    what="Catastrophic backtracking demonstration",
)
