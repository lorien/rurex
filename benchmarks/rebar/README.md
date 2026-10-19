# rurex in rebar

[rebar](https://github.com/BurntSushi/rebar) is BurntSushi's harness that
compares regex engines on a curated set of tasks. This directory holds
the two files needed to measure `rurex` with it: the runner program and a
script that registers the engines.

`main.py` is rebar's `engines/python/main.py` adapted to `rurex`, with one
addition: a `rurex-bytes` mode that hands the binding UTF-8 bytes even
when the benchmark asks for Unicode mode, so the cost of materialising
UTF-8 can be observed on the same content.

## Setup

    git clone https://github.com/BurntSushi/rebar
    cp benchmarks/rebar/main.py rebar/engines/python/rurex/main.py  # mkdir first
    # paste the two [[engine]] blocks below into rebar/benchmarks/engines.toml
    python3 benchmarks/rebar/add_engines.py rebar
    cd rebar && cargo build --release
    ./target/release/rebar build -e '^(python/re|python/rurex|python/rurex-bytes|rust/regex)$'

`engines.toml` (append next to the other Python engines; `python3` is the
host interpreter, `rurex==0.2.0` the released wheel):

    [[engine]]
      name = "python/rurex"
      cwd = "../engines/python/rurex"
      [engine.version]
        bin = "ve/bin/pip"
        args = ["show", "rurex"]
        regex = '(?m)^Version: (?P<version>.+)$'
      [engine.run]
        bin = "ve/bin/python"
        args = ["main.py", "rurex"]
      [[engine.dependency]]
        bin = "python3"
        args = ["--version"]
        regex = '(?m)^Python 3\.'
      [[engine.build]]
        bin = "python3"
        args = ["-m", "venv", "ve"]
      [[engine.build]]
        bin = "ve/bin/pip"
        args = ["install", "rurex==0.2.0"]
      [[engine.clean]]
        bin = "rm"
        args = ["-rf", "./ve"]

    [[engine]]
      name = "python/rurex-bytes"
      cwd = "../engines/python/rurex"
      [engine.version]
        bin = "ve/bin/pip"
        args = ["show", "rurex"]
        regex = '(?m)^Version: (?P<version>.+)$'
      [engine.run]
        bin = "ve/bin/python"
        args = ["main.py", "rurex-bytes"]
      [[engine.dependency]]
        bin = "python3"
        args = ["--version"]
        regex = '(?m)^Python 3\.'
      [[engine.build]]
        bin = "python3"
        args = ["-m", "venv", "ve"]
      [[engine.build]]
        bin = "ve/bin/pip"
        args = ["install", "rurex==0.2.0"]
      [[engine.clean]]
        bin = "rm"
        args = ["-rf", "./ve"]

## Running

Published numbers come from the benchmark host, under the core lease and
pinned to the first physical core (see "Benchmarks" in
`spec/docs/dev.md`):

    python3 benchmarks/corelock.py -- taskset -c 0 \
        /path/to/rebar/target/release/rebar measure \
        -d /path/to/rebar/benchmarks \
        -e '^(python/re|python/rurex|python/rurex-bytes|rust/regex)$' \
        -f '^curated/' --max-time 3s --max-iters 10 > curated.csv
    /path/to/rebar/target/release/rebar rank curated.csv \
        -e '^(python/re|python/rurex|rust/regex)$' --intersection

## What the two modes mean

rebar's `unicode` flag decides the haystack's representation per
benchmark, not the engine: `python/rurex` mirrors `python/re` exactly
(`str` in Unicode mode, `bytes` otherwise), so it is listed wherever
`python/re` is. `python/rurex-bytes` forces the byte engine, which is
only sound where both engines agree on the match count — an ASCII
haystack on a Unicode-mode benchmark. rebar has one such benchmark,
`dictionary/search/english-15`; on non-ASCII haystacks (the Russian and
Chinese words benchmarks) the byte engine would match different counts,
so the str-versus-bytes split there is measured by
`docs/telegram-benchmark.md`, not by rebar.
