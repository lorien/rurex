//! rurex: Python bindings to the Rust `regex` crate.

use pyo3::prelude::*;

mod error;
mod flags;
mod match_;
mod regex;
mod template;

use match_::Match;
use regex::{FindIter, Regex};

/// Escape all regex metacharacters in `text`.
#[pyfunction]
fn escape(text: &str) -> String {
    escape_str(text)
}

fn escape_str(text: &str) -> String {
    ::regex::escape(text)
}

/// Compile a regular expression into a `rurex.Regex`.
#[pyfunction]
#[pyo3(signature = (pattern, flags = 0))]
fn compile(pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Regex> {
    Regex::build(pattern, flags)
}

#[pymodule]
fn rurex(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(escape, m)?)?;
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_class::<Regex>()?;
    m.add_class::<Match>()?;
    m.add_class::<FindIter>()?;
    m.add("error", m.py().get_type::<error::Error>())?;
    // Flag constants at stdlib `re` bit values (ADR-0004, revised).
    for (name, value) in [
        ("I", flags::IGNORECASE),
        ("IGNORECASE", flags::IGNORECASE),
        ("M", flags::MULTILINE),
        ("MULTILINE", flags::MULTILINE),
        ("S", flags::DOTALL),
        ("DOTALL", flags::DOTALL),
        ("U", flags::UNICODE),
        ("UNICODE", flags::UNICODE),
        ("X", flags::VERBOSE),
        ("VERBOSE", flags::VERBOSE),
        ("A", flags::ASCII),
        ("ASCII", flags::ASCII),
    ] {
        m.add(name, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::escape_str;

    #[test]
    fn escapes_metacharacters() {
        assert_eq!(escape_str("a.b*c"), r"a\.b\*c");
        assert_eq!(escape_str("plain"), "plain");
        assert_eq!(escape_str("(a)"), r"\(a\)");
    }
}
