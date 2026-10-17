"""Layer B — parity: rurex vs stdlib `re` on the vendored rust-regex
corpus inputs (ADR-0010).

Run explicitly (not collected by a plain `pytest`):

    python -m pytest tests/rustregex/vendored_corpus_parity.py -q

Ledger discipline as layer A (EXPECTED.md, both directions).
"""

import pytest

from corpus import load_cases
from expected import LEDGER
from parity import run_case

CASES = load_cases()


@pytest.mark.parametrize("case", CASES, ids=lambda c: c.case_id)
def test_parity(case):
    status, detail = run_case(case)
    entry = LEDGER.get((case.case_id, "B"))
    if status == "pass":
        assert entry is None, (
            f"stale ledger row: {case.case_id} passes on layer B "
            f"(was: {entry[0]}: {entry[1]}) — remove the row "
            f"(python gen_expected.py)"
        )
        return
    assert entry is not None, (
        f"{status} on layer B with no ledger row: {detail} — triage "
        f"into EXPECTED.md (python gen_expected.py)"
    )
