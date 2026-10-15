# PyO3 + maturin toolchain

Date: 2026-10-15
Status: accepted

## Context

The binding layer must bridge the Rust `regex` crate to CPython: expose
Rust types as Python objects, convert arguments and results, raise
exceptions, and package the result as an installable module. Several
bridges exist, with different maturity and ergonomics.

## Decision

Use **PyO3** for the binding layer and **maturin** for building and
packaging:

- PyO3 provides `#[pyclass]`/`#[pymethods]`/`#[pymodule]`, `PyResult`
  error conversion, GIL release (`allow_threads`), and is the de-facto
  standard for Rust-to-CPython bindings, with a large ecosystem and
  active maintenance.
- maturin drives `cargo` from `pyproject.toml`, installs the built
  extension into a virtualenv with `maturin develop`, and produces
  wheels (including abi3) for release.

## Alternatives rejected

- **rust-cpython**: predecessor of PyO3, effectively unmaintained.
- **Raw C ABI + ctypes/cffi on the Python side**: full manual control,
  but every conversion becomes hand-written unsafe glue; error handling
  and object lifetimes are error-prone, and packaging stays manual.
- **UniFFI**: strong for multi-language foreign interfaces, but its
  Python story is proxy-object oriented and less ergonomic for exposing
  a tight, `re`-like native API.
- **Subinterpreters / WASM bridge approaches**: unnecessary indirection
  for a native extension and adds runtime constraints.
