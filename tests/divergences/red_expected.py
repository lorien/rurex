"""Parser for the EXPECTED.md ledger.

Exposes LEDGER: {construct_id: bucket} for every construct that does
not simply pass. Buckets: `by-design: <family>` (mode L mismatch in a
documented family), `discuss` (mode E mismatch not explained by a
documented adjustment), `unmapped`.
"""

from __future__ import annotations

import pathlib

_LEDGER_PATH = pathlib.Path(__file__).parent / "EXPECTED.md"

LEDGER: dict[str, str] = {}


def _load():
    for line in _LEDGER_PATH.read_text().splitlines():
        if not line.startswith("| "):
            continue
        parts = [p.strip() for p in line.strip("|").split("|")]
        if len(parts) == 2 and parts[0] not in ("case", ""):
            LEDGER[parts[0]] = parts[1]


_load()
