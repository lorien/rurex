PYTHON := .venv/bin/python
FREE_PYTHON := .venv-t/bin/python
VENV := $(CURDIR)/.venv
FREE_VENV := $(CURDIR)/.venv-t
MATURIN := $(PYTHON) -m maturin

venv:
	python3 -m venv .venv
	.venv/bin/python -m pip install --upgrade pip
	.venv/bin/python -m pip install maturin pytest matplotlib markdown py-spy

# `build` is the optimized build: the project is benchmark-driven, and a
# debug extension left installed silently makes every measurement wrong.
build: build-release

build-debug:
	VIRTUAL_ENV=$(VENV) $(MATURIN) develop

build-release:
	VIRTUAL_ENV=$(VENV) $(MATURIN) develop --release

# Optional: the free-threaded venv is used by the Telegram benchmark
# when it exists. Missing it is not an error.
build-free:
	@if [ -x $(FREE_PYTHON) ]; then \
		VIRTUAL_ENV=$(FREE_VENV) $(FREE_PYTHON) -m maturin develop --release; \
	else \
		echo "no $(FREE_PYTHON); skipping free-threaded build"; \
	fi

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

# Final checks: leave a release build installed so a benchmark run
# afterwards measures the optimized extension, not a debug one.
test-all: build-release test test-conformance

fmt:
	cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings

rust-test:
	cargo test

check: fmt clippy rust-test test-all

bench: build-release
	$(PYTHON) benchmarks/run.py
	$(PYTHON) benchmarks/make_charts.py

bench-telegram: build-release build-free
	$(PYTHON) benchmarks/telegram.py
	$(PYTHON) benchmarks/telegram_charts.py

bench-html: build-release build-free
	$(PYTHON) benchmarks/frontend_html.py
	$(PYTHON) benchmarks/frontend_html_charts.py

.PHONY: venv build build-debug build-release build-free test \
	test-conformance test-all fmt clippy rust-test check bench \
	bench-telegram bench-html
