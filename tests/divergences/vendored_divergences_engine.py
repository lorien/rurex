"""Mode E — engine-contract discussion report on the vendored
regex-engine-divergences corpus (ADR-0011).

Run explicitly (not collected by a plain `pytest`):

    python -m pytest tests/divergences/vendored_divergences_engine.py -q

This is *not* a pass/fail oracle: mismatches vs the upstream-recorded
Rust answers are classified (documented adjustment / version drift /
discuss). Only `discuss` rows require a ledger entry; every other
non-pass status is reported, never failed.
"""

import pytest

from red_corpus import load_constructs
from red_engine import run_case
from red_expected import LEDGER

CONSTRUCTS = load_constructs()


@pytest.mark.parametrize("c", CONSTRUCTS, ids=lambda c: c.cid)
def test_engine_contract(c):
    status, detail = run_case(c)
    if status == "pass":
        assert LEDGER.get(c.cid) != "discuss", (
            f"stale ledger row: {c.cid} matches the recorded Rust "
            f"answers — remove the discuss row"
        )
        return
    if status == "mismatch-discuss":
        assert LEDGER.get(c.cid) == "discuss", (
            f"{c.cid}: unexplained divergence from the engine "
            f"contract: {detail} — triage into EXPECTED.md "
            f"(python gen_expected.py)"
        )
    # mismatch-adjusted / mismatch-unmapped: reported via the ledger
    # generator; not failures by design (ADR-0011).
