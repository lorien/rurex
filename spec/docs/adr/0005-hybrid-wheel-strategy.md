# Hybrid wheel strategy: version-specific + abi3 fallback

Date: 2026-10-15
Status: accepted

## Context

The owner's priority for the compiled extension is maximum runtime
speed. Separately, rolling-release distributions (Arch Linux ships only
the newest CPython minor) punish exact-match-only wheels: in the gap
between a new CPython release and our next wheel refresh, users there
fall back to building the sdist with a Rust toolchain. abi3 wheels are
forward-compatible across CPython versions but route calls through the
stable-ABI trampoline and restrict some PyO3 APIs.

## Decision

Ship both wheel flavors from one source:

- Default builds are version-specific (no `abi3` feature): the
  extension compiles against each supported CPython minor's full
  native API — the fastest available entry points, and pip's preferred
  tags when they match.
- A cargo feature `abi3 = ["pyo3/abi3-py310"]` (off by default) builds
  one abi3-py310 fallback wheel per platform, which pip selects only
  when no exact-match wheel exists — covering brand-new CPython
  releases without a source build.

The release profile is speed-tuned: `lto = "fat"`,
`codegen-units = 1`, `opt-level = 3`, `strip = true`; panics keep
unwinding because PyO3's panic catching depends on it. The first real
release version is 0.1.0, superseding the 0.0.1 placeholder that holds
the PyPI name (ADR-0003). Free-threaded CPython wheels remain a
publish-task question; they would also be version-specific, never
abi3.

## Alternatives rejected

- **Pure abi3-py310** — simplest matrix, but pays the stable-ABI
  trampoline on every call forever and restricts PyO3 APIs; rejected
  on the speed priority.
- **Pure version-specific** — maximum speed, but rolling-distro users
  hit source builds in every new-CPython gap window.
- **Raising the abi3 floor (py311/py312)** — buys nothing we use and
  narrows compatibility.
