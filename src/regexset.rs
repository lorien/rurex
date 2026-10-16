//! The `RegexSet` type: single-pass multi-pattern membership matching.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyList, PyString};

use crate::error;
use crate::flags;
use crate::regex::{Hay, extract_hay, validate_pattern};

enum SetInner {
    Text(Box<regex::RegexSet>),
    Bytes(Box<regex::bytes::RegexSet>),
}

/// A set of patterns matched against text in a single pass; reports
/// membership only (no offsets or captures). Construct via
/// `rurex.compile_set`.
#[pyclass(name = "RegexSet", module = "rurex", frozen)]
pub struct RegexSet {
    inner: SetInner,
    bytes_mode: bool,
}

/// Shared build path for `rurex.compile_set`.
pub(crate) fn build(py: Python<'_>, patterns: &Bound<'_, PyAny>, flags: u32) -> PyResult<RegexSet> {
    // Collect patterns from any iterable, enforcing one mode (the
    // first item's type decides).
    let mut texts: Vec<String> = Vec::new();
    let mut mode: Option<bool> = None;
    let iter = patterns
        .try_iter()
        .map_err(|_| PyTypeError::new_err("patterns must be an iterable of str or bytes"))?;
    for item in iter {
        let item = item?;
        let is_str = item.cast::<PyString>().is_ok();
        let is_bytes = !is_str && item.cast::<PyBytes>().is_ok();
        if !is_str && !is_bytes {
            return Err(PyTypeError::new_err(format!(
                "patterns must be str or bytes, not {}",
                item.get_type()
            )));
        }
        match (mode, is_bytes) {
            (Some(true), false) | (Some(false), true) => {
                return Err(PyTypeError::new_err(
                    "patterns must be all str or all bytes",
                ));
            }
            _ => mode = Some(is_bytes),
        }
        let text: String = if is_bytes {
            let bytes = item
                .cast::<PyBytes>()
                .expect("checked above")
                .as_bytes()
                .to_vec();
            match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => {
                    return Err(error::make_error(
                        py,
                        "pattern is not valid UTF-8",
                        &item,
                        None,
                        None,
                        None,
                    )?);
                }
            }
        } else {
            item.cast::<PyString>()
                .expect("checked above")
                .to_str()
                .map_err(|_| PyTypeError::new_err("invalid str pattern"))?
                .to_string()
        };
        texts.push(text);
    }
    let bytes_mode = mode.unwrap_or(false);

    let cfg = flags::translate(flags, bytes_mode).map_err(PyValueError::new_err)?;

    // Per-pattern pre-parse for structured error positions; the error's
    // `pattern` attribute is the individual failing pattern.
    for (item, text) in patterns
        .try_iter()
        .map_err(|_| PyTypeError::new_err("patterns must be an iterable of str or bytes"))?
        .zip(&texts)
    {
        validate_pattern(py, &item?, text, &cfg, bytes_mode)?;
    }

    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let inner = if bytes_mode {
        let built = regex::bytes::RegexSetBuilder::new(refs)
            .case_insensitive(cfg.case_insensitive)
            .multi_line(cfg.multi_line)
            .dot_matches_new_line(cfg.dot_matches_new_line)
            .ignore_whitespace(cfg.ignore_whitespace)
            .unicode(cfg.unicode)
            .build();
        let set = match built {
            Ok(set) => set,
            Err(e) => {
                return Err(error::make_error(
                    py,
                    &e.to_string(),
                    patterns,
                    None,
                    None,
                    None,
                )?);
            }
        };
        SetInner::Bytes(Box::new(set))
    } else {
        let built = regex::RegexSetBuilder::new(refs)
            .case_insensitive(cfg.case_insensitive)
            .multi_line(cfg.multi_line)
            .dot_matches_new_line(cfg.dot_matches_new_line)
            .ignore_whitespace(cfg.ignore_whitespace)
            .unicode(cfg.unicode)
            .build();
        let set = match built {
            Ok(set) => set,
            Err(e) => {
                return Err(error::make_error(
                    py,
                    &e.to_string(),
                    patterns,
                    None,
                    None,
                    None,
                )?);
            }
        };
        SetInner::Text(Box::new(set))
    };
    Ok(RegexSet { inner, bytes_mode })
}

#[pymethods]
impl RegexSet {
    /// True if any pattern in the set matches `text`.
    fn is_match(&self, py: Python<'_>, text: &Bound<'_, PyAny>) -> PyResult<bool> {
        let keep = extract_hay(self.bytes_mode, text)?;
        match keep.hay()? {
            Hay::Text(s) => match &self.inner {
                SetInner::Text(set) => Ok(py.detach(|| set.is_match(s))),
                SetInner::Bytes(_) => unreachable!(),
            },
            Hay::Bytes(b) => match &self.inner {
                SetInner::Bytes(set) => Ok(py.detach(|| set.is_match(b))),
                SetInner::Text(_) => unreachable!(),
            },
        }
    }

    /// One boolean per pattern, in pattern order.
    fn matches<'py>(
        &self,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let keep = extract_hay(self.bytes_mode, text)?;
        let flags_vec: Vec<bool> = match keep.hay()? {
            Hay::Text(s) => match &self.inner {
                SetInner::Text(set) => {
                    let found = py.detach(|| set.matches(s));
                    (0..found.len()).map(|i| found.matched(i)).collect()
                }
                SetInner::Bytes(_) => unreachable!(),
            },
            Hay::Bytes(b) => match &self.inner {
                SetInner::Bytes(set) => {
                    let found = py.detach(|| set.matches(b));
                    (0..found.len()).map(|i| found.matched(i)).collect()
                }
                SetInner::Text(_) => unreachable!(),
            },
        };
        PyList::new(py, flags_vec)
    }
}
