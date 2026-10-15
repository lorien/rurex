# rurex

Python bindings to the Rust [`regex`](https://crates.io/crates/regex)
crate: fast, safe, linear-time regular expressions for Python, with an
API modeled on the standard `re` module.

Status: under development toward 0.1.0. The design lives in
[`spec/docs/`](spec/docs/index.md); the API contract is
[`spec/docs/api.md`](spec/docs/api.md).

## Development

```sh
python -m venv .venv
source .venv/bin/activate
pip install maturin pytest
maturin develop
pytest
```

See `spec/docs/dev.md` for the full workflow.
