"""Loader for the vendored regex-engine-divergences corpus
(ADR-0011). See PROVENANCE.md for the pinned snapshot and the exact
operation the corpus encodes: compile with i/m/s/x flag mapping
(everything else ignored), finditer over the whole test text, report
(match text, start index) per match plus a compile-verdict flag.

This module is pytest-free so triage scripts can import it directly.
"""

from __future__ import annotations

import json
import pathlib

DATA_DIR = pathlib.Path(__file__).parent / "data"

# Python-visible flag bits (value-compatible with re).
IGNORECASE = 2
MULTILINE = 8
DOTALL = 16
VERBOSE = 64


def flag_bits(flags_str: str) -> int:
    """Upstream's mapping: i/m/s/x only; other letters (g, u, y, ...)
    are ignored by their runners and therefore by ours."""
    bits = 0
    for ch in flags_str:
        if ch == "i":
            bits |= IGNORECASE
        elif ch == "m":
            bits |= MULTILINE
        elif ch == "s":
            bits |= DOTALL
        elif ch == "x":
            bits |= VERBOSE
    return bits


class Construct:
    __slots__ = ("cid", "category", "level", "description",
                 "pattern", "test_text", "flags")

    def __init__(self, raw):
        self.cid = raw["id"]
        self.category = raw.get("category", "")
        self.level = raw.get("level", 0)
        self.description = raw.get("description", "")
        self.pattern = raw["pattern"]
        self.test_text = raw["testText"]
        self.flags = raw.get("flags", "")

    def __repr__(self):  # pragma: no cover
        return f"<Construct {self.cid}>"


def load_constructs():
    doc = json.loads((DATA_DIR / "constructs.json").read_text())
    return [Construct(raw) for raw in doc]


def load_recorded(which: str) -> dict:
    """Upstream's recorded answers: {construct id: {"matches": [...],
    "error": str | None}}. Triage context only — never an oracle."""
    return json.loads((DATA_DIR / f"{which}.json").read_text())
