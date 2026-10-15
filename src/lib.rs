//! rurex: Python bindings to the Rust `regex` crate.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyString};

mod error;
mod flags;
mod match_;
mod regex;
mod regexset;
mod template;

use match_::Match;
use regex::{FindIter, Regex};
use regexset::RegexSet;

/// Maximum entries in the module pattern cache, like `re._MAXCACHE`.
const MAX_CACHE: usize = 512;

type CacheKey = (bool, Vec<u8>, u32);

#[derive(Default)]
struct CacheState {
    map: HashMap<CacheKey, Py<Regex>>,
    order: VecDeque<CacheKey>,
}

impl CacheState {
    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

static CACHE: std::sync::LazyLock<Mutex<CacheState>> = std::sync::LazyLock::new(|| {
    Mutex::new(CacheState {
        map: HashMap::new(),
        order: VecDeque::new(),
    })
});

/// Compile (or fetch from the module cache) a pattern for a
/// module-level convenience function. Mirrors CPython's `re._compile`:
/// FIFO eviction at 512 entries, keyed by pattern type, text, and
/// flags.
fn cached_compile(py: Python<'_>, pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Py<Regex>> {
    let key: CacheKey = if let Ok(s) = pattern.cast::<PyString>() {
        (
            false,
            s.to_str()
                .map_err(|_| PyTypeError::new_err("invalid str pattern"))?
                .as_bytes()
                .to_vec(),
            flags,
        )
    } else if let Ok(b) = pattern.cast::<PyBytes>() {
        (true, b.as_bytes().to_vec(), flags)
    } else {
        return Err(PyTypeError::new_err(format!(
            "pattern must be str or bytes, not {}",
            pattern.get_type()
        )));
    };
    {
        let cache = CACHE.lock().expect("pattern cache lock");
        if let Some(rx) = cache.map.get(&key) {
            return Ok(rx.clone_ref(py));
        }
    }
    let rx = Py::new(py, Regex::build(pattern, flags)?)?;
    let mut cache = CACHE.lock().expect("pattern cache lock");
    if cache.map.len() >= MAX_CACHE {
        if let Some(oldest) = cache.order.pop_front() {
            cache.map.remove(&oldest);
        }
    }
    cache.map.insert(key.clone(), rx.clone_ref(py));
    cache.order.push_back(key);
    Ok(rx)
}

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

/// Compile many patterns into a `rurex.RegexSet` that scans for all of
/// them in a single pass (membership only).
#[pyfunction]
#[pyo3(signature = (patterns, flags = 0))]
fn compile_set(patterns: &Bound<'_, PyAny>, flags: u32) -> PyResult<RegexSet> {
    regexset::build(patterns.py(), patterns, flags)
}

/// Clear the module pattern cache (like `re.purge`).
#[pyfunction]
fn purge() {
    CACHE.lock().expect("pattern cache lock").clear();
}

#[pyfunction]
#[pyo3(signature = (pattern, text, flags = 0))]
fn search<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(rx
        .bind(py)
        .call_method("search", (text,), None)?
        .extract()?)
}

/// `re.match` under its Python 3.15 name.
#[pyfunction]
#[pyo3(signature = (pattern, text, flags = 0))]
fn prefixmatch<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(rx
        .bind(py)
        .call_method("prefixmatch", (text,), None)?
        .extract()?)
}

#[pyfunction]
#[pyo3(signature = (pattern, text, flags = 0))]
fn fullmatch<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(rx
        .bind(py)
        .call_method("fullmatch", (text,), None)?
        .extract()?)
}

#[pyfunction]
#[pyo3(signature = (pattern, text, flags = 0))]
fn findall<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    rx.bind(py).call_method("findall", (text,), None)
}

#[pyfunction]
#[pyo3(signature = (pattern, text, flags = 0))]
fn finditer(
    py: Python<'_>,
    pattern: &Bound<'_, PyAny>,
    text: &Bound<'_, PyAny>,
    flags: u32,
) -> PyResult<Py<FindIter>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(rx
        .bind(py)
        .call_method("finditer", (text,), None)?
        .extract()?)
}

#[pyfunction]
#[pyo3(signature = (pattern, text, maxsplit = 0, flags = 0))]
fn split<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    maxsplit: usize,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    rx.bind(py).call_method("split", (text, maxsplit), None)
}

#[pyfunction]
#[pyo3(signature = (pattern, repl, text, count = 0, flags = 0))]
fn sub<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    repl: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    count: usize,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    rx.bind(py).call_method("sub", (repl, text, count), None)
}

#[pyfunction]
#[pyo3(signature = (pattern, repl, text, count = 0, flags = 0))]
fn subn(
    py: Python<'_>,
    pattern: &Bound<'_, PyAny>,
    repl: &Bound<'_, PyAny>,
    text: &Bound<'_, PyAny>,
    count: usize,
    flags: u32,
) -> PyResult<(Py<PyAny>, usize)> {
    let rx = cached_compile(py, pattern, flags)?;
    rx.bind(py)
        .call_method("subn", (repl, text, count), None)?
        .extract()
}

#[pymodule]
fn rurex(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(escape, m)?)?;
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(compile_set, m)?)?;
    m.add_function(wrap_pyfunction!(purge, m)?)?;
    m.add_function(wrap_pyfunction!(search, m)?)?;
    m.add_function(wrap_pyfunction!(prefixmatch, m)?)?;
    m.add_function(wrap_pyfunction!(fullmatch, m)?)?;
    m.add_function(wrap_pyfunction!(findall, m)?)?;
    m.add_function(wrap_pyfunction!(finditer, m)?)?;
    m.add_function(wrap_pyfunction!(split, m)?)?;
    m.add_function(wrap_pyfunction!(sub, m)?)?;
    m.add_function(wrap_pyfunction!(subn, m)?)?;
    m.add_class::<Regex>()?;
    m.add_class::<RegexSet>()?;
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
