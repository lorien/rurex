"""Benchmark suite: single-variable benchmark tests.

Each test varies exactly one parameter and states its fixed context in
the title; a test op maps to one operation on a precompiled pattern:

- "search":        rx.search(text)
- "findall":       rx.findall(text)
- "guarded_search": single-shot, timeout-guarded (re explodes)
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
    """Digit runs (\\d{3,4}-shaped) embedded in word filler at a given
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

SCAN_POSITION_FIXED_KB = 8192
EXTRACT_SIZE_FIXED_DENSITY = 80
EXTRACT_DENSITY_FIXED_KB = 1024
THREADS_DOC_KB = 1024
THREADS_PILE = 8000
THREADS_POS_FIXED_KB = SCAN_POSITION_FIXED_KB
THREADS_POS_WORKERS = 4
THREADS_POS_PILE = 1000


def _example(text, width=110):
    return text[:width].rstrip() + " …"


TESTS = [
    {
        "name": "scan_size",
        "title": "Test 1 — Scan: haystack size (needle at the end)",
        "description": (
            "A plain literal search for a single needle occurring once, "
            "with the needle at the very end of the text — the worst "
            "case, the whole haystack must be scanned. Variable: "
            "haystack size from 64 KB to 16 MB. This is the classic "
            "log-grep workload: rurex scans with SIMD-accelerated "
            "prefilters, `re` steps its backtracking VM character by "
            "character."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": _example(
            with_needle_at(4096, SCAN_PATTERN, 100)
        ),
        "op": "search",
        "sweeps": [
            {
                "param": "data_size",
                "unit": "KB",
                "label": "haystack size, needle at the end (worst case)",
                "values": [64, 256, 1024, 4096, 16384],
                "make": lambda kb: with_needle_at(kb * 1024, SCAN_PATTERN, 100),
            },
        ],
    },
    {
        "name": "scan_position",
        "title": (
            f"Test 2 — Scan: needle position "
            f"(fixed {SCAN_POSITION_FIXED_KB // 1024} MB haystack)"
        ),
        "description": (
            "The same single-needle literal search as Test 1, but the "
            "haystack is fixed at 8 MB and the variable is where in the "
            "text the needle sits, from the first bytes (0%) to the very "
            "end (100%). Position 0% favors `re`'s instant fast path; "
            "every later position is scan-bound."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": (
            _example(words(60, random.Random(2)))
            + " " + SCAN_PATTERN + " "
            + _example(words(60, random.Random(3)))
        ),
        "op": "search",
        "sweeps": [
            {
                "param": "match_position",
                "unit": "%",
                "label": "needle position in the fixed 8 MB haystack",
                "values": [0, 25, 50, 75, 100],
                "make": lambda pct: with_needle_at(
                    SCAN_POSITION_FIXED_KB * 1024, SCAN_PATTERN, pct
                ),
            },
        ],
    },
    {
        "name": "extract_size",
        "title": (
            "Test 3 — Extraction: haystack size "
            f"(fixed density, {EXTRACT_SIZE_FIXED_DENSITY} matches/KB)"
        ),
        "description": (
            "Digit-run extraction (`findall`) over word filler at a "
            "fixed density of 80 matches per KB. Variable: haystack size "
            "from 64 KB to 4 MB (the match count grows with it). This "
            "is the match-dense regime, where building Python match "
            "objects — not scanning — dominates."
        ),
        "pattern": EXTRACT_PATTERN,
        "data_example": _example(numbers_with_density(2048, 80)),
        "op": "findall",
        "sweeps": [
            {
                "param": "data_size",
                "unit": "KB",
                "label": "haystack size at fixed density (80 matches/KB)",
                "values": [64, 256, 1024, 4096],
                "make": lambda kb: numbers_with_density(
                    kb * 1024, EXTRACT_SIZE_FIXED_DENSITY
                ),
            },
        ],
    },
    {
        "name": "extract_density",
        "title": (
            "Test 4 — Extraction: match density "
            f"(fixed {EXTRACT_DENSITY_FIXED_KB // 1024} MB haystack)"
        ),
        "description": (
            "The same digit-run extraction as Test 3, but the haystack "
            "is fixed at 1 MB and the variable is match density, from 5 "
            "to 160 matches per KB. This isolates the per-match cost "
            "from the scan cost and shows the crossover: sparse matches "
            "are scan-bound (rurex territory), dense matches are "
            "extraction-bound (`re` territory)."
        ),
        "pattern": EXTRACT_PATTERN,
        "data_example": _example(numbers_with_density(2048, 160)),
        "op": "findall",
        "sweeps": [
            {
                "param": "match_density",
                "unit": "matches/KB",
                "label": "match density in the fixed 1 MB haystack",
                "values": [5, 20, 80, 160],
                "make": lambda per_kb: numbers_with_density(
                    EXTRACT_DENSITY_FIXED_KB * 1024, per_kb
                ),
            },
        ],
    },
    {
        "name": "pathological",
        "title": "Test 5 — Pathological: catastrophic backtracking",
        "description": (
            "The pattern `(x+x+)+y` never matches a run of x's, and for "
            "a backtracking engine the number of ways to distribute the "
            "x's across the groups grows exponentially: `re`'s time "
            "doubles with every two extra characters. A finite-automata "
            "engine cannot backtrack, so rurex' time is flat. This is a "
            "safety property (ReDoS resistance), not a speed feature."
        ),
        "pattern": PATHO_PATTERN,
        "data_example": "x" * 24 + "  (n = 24 of the sweep; no y, so nothing matches)",
        "op": "guarded_search",
        "sweeps": [
            {
                "param": "input_length",
                "unit": "chars",
                "label": "input length n, text = \"x\" * n",
                "values": list(range(8, 27, 2)),
                "values_rurex": list(range(8, 29, 4)),
                "make": lambda n: "x" * n,
            },
        ],
    },
    {
        "name": "threads_gil",
        "title": "Test 6 — Concurrency: threads on GIL Python",
        "description": (
            "One fixed pile of 8000 whole-haystack searches (1 MB "
            "each, needle at the end), split across N worker threads; "
            "the chart shows wall-clock time to finish the whole "
            "pile. On GIL Python no engine spends threads on "
            "throughput: `re` serializes on the GIL (its per-op time "
            "inflates with the thread count), and rurex — far faster "
            "per op at every thread count — flattens once the "
            "GIL-held part of each call (Match construction) "
            "dominates its fast scans. Threads pay off only on "
            "free-threaded Python (Test 7)."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": _example(with_needle_at(4096, SCAN_PATTERN, 100)),
        "op": "threaded_search",
        "interpreter": "gil",
        "pile": THREADS_PILE,
        "sweeps": [
            {
                "param": "threads",
                "unit": "workers",
                "label": (
                    "worker threads sharing one fixed pile of "
                    f"{THREADS_PILE} searches over a {THREADS_DOC_KB} KB "
                    "haystack (needle at the end)"
                ),
                "values": [1, 2, 4, 8],
                "make": lambda n: with_needle_at(
                    THREADS_DOC_KB * 1024, SCAN_PATTERN, 100
                ),
            },
        ],
    },
    {
        "name": "threads_free",
        "title": "Test 7 — Concurrency: threads on free-threaded Python",
        "description": (
            "The same pile as Test 6 — 8000 whole-haystack searches "
            "(1 MB each), split across N worker threads, wall-clock "
            "time for the whole pile — run on free-threaded CPython "
            "(3.14t, GIL off). With no GIL both engines turn threads "
            "into throughput (4-5x at 8 workers on this 4-core/"
            "8-thread machine), and rurex keeps its single-thread "
            "lead at every point."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": _example(with_needle_at(4096, SCAN_PATTERN, 100)),
        "op": "threaded_search",
        "interpreter": "freethreaded",
        "pile": THREADS_PILE,
        "sweeps": [
            {
                "param": "threads",
                "unit": "workers",
                "label": (
                    "worker threads sharing one fixed pile of "
                    f"{THREADS_PILE} searches over a {THREADS_DOC_KB} KB "
                    "haystack (needle at the end), GIL off"
                ),
                "values": [1, 2, 4, 8],
                "make": lambda n: with_needle_at(
                    THREADS_DOC_KB * 1024, SCAN_PATTERN, 100
                ),
            },
        ],
    },
    {
        "name": "threads_pos_gil",
        "title": (
            "Test 8 — Concurrency: needle position on GIL Python "
            f"({THREADS_POS_WORKERS} workers)"
        ),
        "description": (
            "Test 2's scan workload as one fixed pile — "
            f"{THREADS_POS_PILE} whole-haystack searches over the "
            f"{THREADS_POS_FIXED_KB // 1024} MB haystack, split across "
            f"{THREADS_POS_WORKERS} barrier-started workers — with the "
            "needle's position as the variable (25–100% of the text). "
            "Position 0% is excluded: a ~170 ns `re` op cannot fill a "
            "thread pile (thread startup alone costs milliseconds), so "
            "that point would time the harness, not the engines. On "
            "GIL Python `re` serializes on the GIL at every position, "
            "while rurex's GIL-released scans overlap — until the "
            "GIL-held Match-construction tail and memory bandwidth "
            "(the 8 MB doc exceeds this CPU's L3) flatten its curve."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": (
            _example(words(60, random.Random(2)))
            + " " + SCAN_PATTERN + " "
            + _example(words(60, random.Random(3)))
        ),
        "op": "threaded_search",
        "interpreter": "gil",
        "pile": THREADS_POS_PILE,
        "threads": THREADS_POS_WORKERS,
        "sweeps": [
            {
                "param": "match_position",
                "unit": "%",
                "label": (
                    "needle position in the fixed 8 MB haystack, "
                    f"{THREADS_POS_WORKERS} workers × a pile of "
                    f"{THREADS_POS_PILE} searches"
                ),
                "values": [25, 50, 75, 100],
                "make": lambda pct: with_needle_at(
                    THREADS_POS_FIXED_KB * 1024, SCAN_PATTERN, pct
                ),
            },
        ],
    },
    {
        "name": "threads_pos_free",
        "title": (
            "Test 9 — Concurrency: needle position on free-threaded "
            f"Python ({THREADS_POS_WORKERS} workers)"
        ),
        "description": (
            "The same pile as Test 8 — "
            f"{THREADS_POS_PILE} searches over the "
            f"{THREADS_POS_FIXED_KB // 1024} MB haystack across "
            f"{THREADS_POS_WORKERS} workers, needle position swept "
            "25–100% — on free-threaded CPython (3.14t, GIL off). With "
            "no GIL both engines turn the workers into scan throughput "
            "at every position; the 8 MB doc exceeds this CPU's L3, so "
            "four parallel scanners share memory bandwidth and scale "
            "below the ideal 4×."
        ),
        "pattern": SCAN_PATTERN,
        "data_example": (
            _example(words(60, random.Random(2)))
            + " " + SCAN_PATTERN + " "
            + _example(words(60, random.Random(3)))
        ),
        "op": "threaded_search",
        "interpreter": "freethreaded",
        "pile": THREADS_POS_PILE,
        "threads": THREADS_POS_WORKERS,
        "sweeps": [
            {
                "param": "match_position",
                "unit": "%",
                "label": (
                    "needle position in the fixed 8 MB haystack, "
                    f"{THREADS_POS_WORKERS} workers × a pile of "
                    f"{THREADS_POS_PILE} searches, GIL off"
                ),
                "values": [25, 50, 75, 100],
                "make": lambda pct: with_needle_at(
                    THREADS_POS_FIXED_KB * 1024, SCAN_PATTERN, pct
                ),
            },
        ],
    },
]
