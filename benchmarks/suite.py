"""Benchmark suite: three tests, each with one or more parameter sweeps.

A test describes a pattern, why it is interesting, a sample of the data
it runs on, and a list of sweeps. A sweep varies one parameter (data
length, match position, density, ...) and measures the time one
operation takes for each engine.

Test op names map to operations on a precompiled pattern:
- "search":  rx.search(text)
- "findall": rx.findall(text)
Pathological cases ("guarded_search") are single-shot and timeout-
guarded in run.py because re's time explodes.
"""

import random


def words(n_bytes, rng, extra=""):
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


def with_needle_at(n_bytes, needle, position_pct):
    """English-ish filler with exactly one needle, placed at a given
    percentage of the text (100 = the very end, the scan worst case)."""
    filler = words(n_bytes - len(needle) - 1, random.Random(42))
    cut = int(len(filler) * position_pct / 100)
    return filler[:cut] + needle + filler[cut:]


def numbers_with_density(n_bytes, matches_per_kb):
    r"""Digit runs (\d{3,4}-shaped) embedded in word filler at a given
    density (matches per KB of text)."""
    rng = random.Random(9)
    filler_v = ["alpha", "beta", "word", "text", "value", "node", "lorem"]
    target = matches_per_kb * (n_bytes // 1024)
    total_matches = max(target, 1)
    parts = []
    size = 0
    emitted = 0
    word_every = max(n_bytes // total_matches, 1)
    while size < n_bytes and emitted < total_matches * 3:
        if emitted < total_matches and size % word_every < 8:
            parts.append(str(rng.randrange(100, 10000)))
            emitted += 1
        else:
            parts.append(rng.choice(filler_v))
        size += len(parts[-1]) + 1
    return " ".join(parts)[:n_bytes]


SCAN_PATTERN = "the-needle-string"
EXTRACT_PATTERN = r"\d{3,4}"
PATHO_PATTERN = r"(x+x+)+y"

SCAN_SIZES_KB = [64, 256, 1024, 4096, 16384]
SCAN_POSITIONS_PCT = [0, 25, 50, 75, 100]
SCAN_POSITION_FIXED_KB = 8192
EXTRACT_SIZES_KB = [64, 256, 1024, 4096]
EXTRACT_DENSITY = [5, 20, 80, 160]
EXTRACT_DENSITY_FIXED_KB = 1024
PATHO_RANGE_RE = range(8, 27, 2)
PATHO_RANGE_RUREX = range(8, 29, 4)


def scan_data_example():
    import random
    filler = words(100, random.Random(1))
    return filler + " " + SCAN_PATTERN + " …"


def extract_data_example():
    return numbers_with_density(400, 80)


def patho_data_example():
    return "x" * 24 + "  (n=24 of the sweep; no y, so nothing ever matches)"


TESTS = [
    {
        "name": "scan",
        "title": "Test 1 — Scan: one literal in a large haystack",
        "description": (
            "A plain literal search for a single needle that occurs once. "
            "This is the classic log-grep workload and the automata "
            "engine's home turf: rurex scans with SIMD-accelerated "
            "prefilters, `re` steps its backtracking VM byte by byte. "
            "Two parameters are swept: haystack length, and where in the "
            "haystack the needle sits (position 100% is the worst case: "
            "the whole text must be scanned)."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": scan_data_example(),
        "op": "search",
        "sweeps": [
            {
                "param": "data_size",
                "unit": "KB",
                "label": "haystack size (needle at the end)",
                "values": SCAN_SIZES_KB,
                "make": lambda kb: with_needle_at(kb * 1024, SCAN_PATTERN, 100),
            },
            {
                "param": "match_position",
                "unit": "%",
                "label": f"needle position in an {SCAN_POSITION_FIXED_KB // 1024} MB haystack",
                "values": SCAN_POSITIONS_PCT,
                "make": lambda pct: with_needle_at(
                    SCAN_POSITION_FIXED_KB * 1024, SCAN_PATTERN, pct
                ),
            },
        ],
    },
    {
        "name": "extract",
        "title": "Test 2 — Extraction: findall over match-dense data",
        "description": (
            "Digit-run extraction (`findall`) over word filler — the "
            "regime where `re` wins today: matches are small and "
            "everywhere, so the cost is dominated by building Python "
            "match objects, and rurex currently constructs each result "
            "through Python-level slicing. Two parameters are swept: "
            "data length at fixed density, and match density at fixed "
            "length (isolating the per-match cost from the scan cost)."
        ),
        "pattern": EXTRACT_PATTERN,
        "data_example": extract_data_example(),
        "op": "findall",
        "sweeps": [
            {
                "param": "data_size",
                "unit": "KB",
                "label": "haystack size (fixed density, 80 matches/KB)",
                "values": EXTRACT_SIZES_KB,
                "make": lambda kb: numbers_with_density(kb * 1024, 80),
            },
            {
                "param": "match_density",
                "unit": "matches/KB",
                "label": "match density in a 1 MB haystack",
                "values": EXTRACT_DENSITY,
                "make": lambda per_kb: numbers_with_density(
                    EXTRACT_DENSITY_FIXED_KB * 1024, per_kb
                ),
            },
        ],
    },
    {
        "name": "pathological",
        "title": "Test 3 — Pathological: catastrophic backtracking",
        "description": (
            "The pattern `(x+x+)+y` never matches a run of x's, and for "
            "a backtracking engine the number of ways to distribute the "
            "x's across the groups grows exponentially: `re`'s time "
            "doubles with every two extra characters. A finite-automata "
            "engine cannot backtrack, so rurex's time grows linearly "
            "with input length — at n=28 it answers in nanoseconds "
            "where `re` would run for longer than the universe's age. "
            "This is a safety property (ReDoS resistance), not a speed "
            "feature."
        ),
        "pattern": PATHO_PATTERN,
        "data_example": patho_data_example(),
        "op": "guarded_search",
        "sweeps": [
            {
                "param": "input_length",
                "unit": "chars",
                "label": "input length n, text = \"x\" * n",
                "values": list(PATHO_RANGE_RE),
                "values_rurex": list(PATHO_RANGE_RUREX),
                "make": lambda n: "x" * n,
            },
        ],
    },
]
