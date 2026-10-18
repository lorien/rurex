PYTHON := .venv/bin/python

venv:
	python3 -m venv .venv
	.venv/bin/python -m pip install --upgrade pip
	.venv/bin/python -m pip install maturin pytest matplotlib markdown py-spy

build:
	$(PYTHON) -m maturin develop

test:
	$(PYTHON) -m pytest tests/ -q

# The CPython harness installs a process-wide `re` shim; the parity
# suites below need the genuine stdlib `re` as their oracle. Two pytest
# invocations = two interpreters. Do not merge these lines.
test-conformance:
	$(PYTHON) -m pytest tests/cpython/vendored_test_re.py -q
	$(PYTHON) -m pytest \
		tests/rustregex/vendored_corpus_engine.py \
		tests/rustregex/vendored_corpus_parity.py \
		tests/divergences/vendored_divergences_live.py \
		tests/divergences/vendored_divergences_engine.py -q

test-all: build test test-conformance

.PHONY: venv build test test-conformance test-all
