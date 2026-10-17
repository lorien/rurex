"""Layer A — binding conformance: rurex vs the vendored rust-regex
corpus expectations (ADR-0010).

Run explicitly (not collected by a plain `pytest`):

    python -m pytest tests/rustregex/vendored_corpus_engine.py -q

Every non-passing case must have a row in EXPECTED.md; every ledger
row whose case passes is stale and fails the suite.
"""

import pytest

from corpus import load_cases
from engine import run_case
from expected import LEDGER

CASES = load_cases()


@pytest.mark.parametrize("case", CASES, ids=lambda c: c.case_id)
def test_engine_conformance(case):
    status, detail = run_case(case)
    entry = LEDGER.get((case.case_id, "A"))
    if status == "pass":
        assert entry is None, (
            f"stale ledger row: {case.case_id} passes on layer A "
            f"(was: {entry[0]}: {entry[1]}) — remove the row "
            f"(python gen_expected.py)"
        )
        return
    assert entry is not None, (
        f"{status} on layer A with no ledger row: {detail} — triage "
        f"into EXPECTED.md (python gen_expected.py)"
    )
