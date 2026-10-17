"""Mode L — live parity vs stdlib `re` on the vendored
regex-engine-divergences corpus (ADR-0011).

Run explicitly (not collected by a plain `pytest`):

    python -m pytest tests/divergences/vendored_divergences_live.py -q

Hard comparison with ledger discipline (EXPECTED.md).
"""

import pytest

from red_corpus import load_constructs
from red_expected import LEDGER
from red_live import run_case

CONSTRUCTS = load_constructs()


@pytest.mark.parametrize("c", CONSTRUCTS, ids=lambda c: c.cid)
def test_live_parity(c):
    status, detail = run_case(c)
    bucket = LEDGER.get(c.cid)
    if status == "pass":
        assert bucket is None, (
            f"stale ledger row: {c.cid} passes vs live re "
            f"(was: {bucket}) — remove it (python gen_expected.py)"
        )
        return
    assert bucket is not None, (
        f"{status} vs live re with no ledger row: {detail} — triage "
        f"into EXPECTED.md (python gen_expected.py)"
    )
