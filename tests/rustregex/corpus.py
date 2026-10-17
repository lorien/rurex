"""Loader for the vendored rust-regex testdata corpus (ADR-0010).

Normalizes the TOML `[[test]]` entries (see PROVENANCE.md for the
pinned tag) into flat `Case` records consumed by the two harness
layers:

- Layer A (`test_corpus_engine.py`): rurex vs the corpus's expected
  byte-offset spans — binding conformance to the engine's semantics.
- Layer B (`test_corpus_parity.py`): rurex vs stdlib `re` on the same
  inputs — behavioral parity between the two Python modules.

This module is deliberately pytest-free so triage scripts can import
it directly.
"""

from __future__ import annotations

import pathlib
import tomllib

DATA_DIR = pathlib.Path(__file__).parent / "data"

# Python-visible flag bit (value-compatible with re.IGNORECASE).
IGNORECASE = 2


class CorpusError(Exception):
    """The corpus uses a form this loader does not implement."""


def _normalize_match(entry):
    """One `matches` element -> (pattern_id, [span-or-None per group]).

    The four TOML forms (regex-test docs):

    - `[5, 12]`                        simple span, id 0
    - `{ id = 5, span = [20, 21] }`    id + span
    - `[[5, 10], [6, 8], [], [9, 10]]` capture-group spans, id 0
      (`[]` = group present but did not participate)
    - `{ id = 5, spans = [...] }`      id + capture-group spans
    """
    if isinstance(entry, list):
        if not entry or isinstance(entry[0], int):
            return (0, [tuple(entry)] if entry else None)
        groups = [tuple(g) if g else None for g in entry]
        return (0, groups)
    pid = entry.get("id", 0)
    if "span" in entry:
        return (pid, [tuple(entry["span"])])
    return (pid, [tuple(g) if g else None for g in entry["spans"]])


def unescape_haystack(text: str) -> bytes:
    r"""Decode escaped haystacks (`unescape = true` tests).

    The escape forms used by this corpus (verified over all 1,203
    cases): `\xNN`, `\\` (backslash), `\n`, `\r`, `\t`. Anything
    else after a backslash passes through unchanged.
    """
    out = bytearray()
    i = 0
    HEX = "0123456789abcdefABCDEF"
    while i < len(text):
        c = text[i]
        if c == "\\" and i + 1 < len(text):
            nxt = text[i + 1]
            if (
                nxt == "x"
                and i + 3 < len(text)
                and text[i + 2] in HEX
                and text[i + 3] in HEX
            ):
                out.append(int(text[i + 2 : i + 4], 16))
                i += 4
                continue
            if nxt == "\\":
                out.append(0x5C)
                i += 2
                continue
            if nxt == "n":
                out.append(0x0A)
                i += 2
                continue
            if nxt == "r":
                out.append(0x0D)
                i += 2
                continue
            if nxt == "t":
                out.append(0x09)
                i += 2
                continue
        out += c.encode("utf-8")
        i += 1
    return bytes(out)


def char_spans(haystack: str, groups):
    """Convert expected byte spans to char spans (str-mode tests)."""
    if groups is None:
        return None
    # Map byte offset -> char offset once.
    b2c = {}
    b = 0
    for c, ch in enumerate(haystack):
        b2c[b] = c
        b += len(ch.encode("utf-8"))
    b2c[b] = len(haystack)

    def conv(span):
        if span is None:
            return None
        try:
            return (b2c[span[0]], b2c[span[1]])
        except KeyError:
            raise CorpusError(f"span {span} not on a char boundary")

    return [conv(s) for s in groups]


def byte_bounds_to_char(haystack: str, bounds):
    """`bounds` (list `[start, end]` or dict `{start, end}`) in bytes
    -> (pos, endpos) in chars."""
    if isinstance(bounds, dict):
        bounds = [bounds["start"], bounds["end"]]
    start, end = bounds
    b2c = {}
    b = 0
    for c, ch in enumerate(haystack):
        b2c[b] = c
        b += len(ch.encode("utf-8"))
    b2c[b] = len(haystack)
    if start not in b2c or end not in b2c:
        raise CorpusError(f"bounds {bounds} not on char boundaries")
    return b2c[start], b2c[end]


class Case:
    """One normalized corpus test case."""

    __slots__ = (
        "case_id", "file", "name", "group", "patterns", "multi",
        "haystack", "unescape", "options", "expected",
    )

    def __init__(self, file, group, name, raw):
        regex = raw["regex"]
        self.multi = isinstance(regex, list)
        self.patterns = regex if self.multi else [regex]
        self.file = file
        self.group = group
        self.name = name
        self.case_id = f"{group}/{name}"
        self.haystack = raw["haystack"]
        self.unescape = bool(raw.get("unescape", False))
        self.options = {
            k: v for k, v in raw.items()
            if k not in ("regex", "haystack", "matches", "unescape", "name")
        }
        self.expected = [
            _normalize_match(m) for m in raw.get("matches", [])
        ]

    def opt(self, key, default=None):
        return self.options.get(key, default)

    def __repr__(self):  # pragma: no cover
        return f"<Case {self.case_id}>"


def load_cases():
    """All cases from the vendored corpus, deterministic order."""
    files = sorted(DATA_DIR.glob("*.toml")) + sorted(
        (DATA_DIR / "fowler").glob("*.toml")
    )
    cases = []
    for path in files:
        group = path.stem
        if group == "README":
            continue
        doc = tomllib.loads(path.read_text())
        for raw in doc["test"]:
            cases.append(
                Case(str(path.relative_to(DATA_DIR)), group, raw["name"], raw)
            )
    return cases
