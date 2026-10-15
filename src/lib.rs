//! rurex: Python bindings to the Rust `regex` crate.

use pyo3::prelude::*;

mod error;

/// Escape all regex metacharacters in `text`.
#[pyfunction]
fn escape(text: &str) -> String {
    escape_str(text)
}

fn escape_str(text: &str) -> String {
    regex::escape(text)
}

#[pymodule]
fn rurex(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(escape, m)?)?;
    m.add("error", m.py().get_type::<error::Error>())?;
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
