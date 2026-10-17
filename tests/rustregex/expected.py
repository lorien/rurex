"""Parser for the EXPECTED.md ledger.

Exposes LEDGER: {(case_id, layer): (bucket, reason)}. Layers: "A" =
binding conformance vs the corpus, "B" = parity vs stdlib `re`.
"""

from __future__ import annotations

import pathlib

_LEDGER_PATH = pathlib.Path(__file__).parent / "EXPECTED.md"

LEDGER: dict[tuple[str, str], tuple[str, str]] = {}


def _load():
    for line in _LEDGER_PATH.read_text().splitlines():
        if not line.startswith("| "):
            continue
        parts = [p.strip() for p in line.strip("|").split("|")]
        if len(parts) == 4 and parts[1] in ("A", "B"):
            LEDGER[(parts[0], parts[1])] = (parts[2], parts[3])


_load()
