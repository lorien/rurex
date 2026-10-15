//! The `Regex` type: a compile-once wrapper over the engine.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyString};

use crate::error;
use crate::flags;

enum Inner {
    Text(Box<regex::Regex>),
    Bytes(Box<regex::bytes::Regex>),
}

enum Pattern {
    Text(String),
    Bytes(Vec<u8>),
}

/// A compiled regular expression; construct via `rurex.compile` (or
/// `Regex(...)` directly).
#[pyclass(name = "Regex", module = "rurex", frozen)]
pub struct Regex {
    inner: Inner,
    pattern: Pattern,
    flags: u32,
}

impl Regex {
    /// Shared compile path for the constructor and `rurex.compile`.
    pub(crate) fn build(pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Self> {
        let py = pattern.py();
        let (pat, bytes_mode) = if let Ok(s) = pattern.extract::<&str>() {
            (Pattern::Text(s.to_owned()), false)
        } else if let Ok(b) = pattern.extract::<&[u8]>() {
            (Pattern::Bytes(b.to_vec()), true)
        } else {
            return Err(PyTypeError::new_err(format!(
                "pattern must be str or bytes, not {}",
                pattern.get_type()
            )));
        };
        let cfg =
            flags::translate(flags, bytes_mode).map_err(pyo3::exceptions::PyValueError::new_err)?;

        // The engine takes pattern text as &str in both modes; byte
        // patterns must be valid UTF-8 (raw byte literals in patterns
        // are not supported; use \xNN escapes).
        let text: &str = match &pat {
            Pattern::Text(s) => s,
            Pattern::Bytes(b) => match std::str::from_utf8(b) {
                Ok(s) => s,
                Err(_) => {
                    return Err(error::make_error(
                        py,
                        "pattern is not valid UTF-8",
                        pattern,
                        None,
                        None,
                        None,
                    )?);
                }
            },
        };

        // Pre-parse with regex-syntax (the engine's own parser) under
        // the same flag configuration, so syntax errors carry
        // structured positions for full `re.error` attribute parity.
        let mut parser = regex_syntax::ParserBuilder::new();
        parser
            .case_insensitive(cfg.case_insensitive)
            .multi_line(cfg.multi_line)
            .dot_matches_new_line(cfg.dot_matches_new_line)
            .ignore_whitespace(cfg.ignore_whitespace)
            .unicode(cfg.unicode)
            .utf8(!bytes_mode);
        if let Err(err) = parser.build().parse(text) {
            // kind()/span() exist on the Parse/Translate variants; the
            // enum is non_exhaustive, so anything else falls back to a
            // position-less error.
            let (msg, offset) = match &err {
                regex_syntax::Error::Parse(e) => {
                    (e.kind().to_string(), Some(e.span().start.offset))
                }
                regex_syntax::Error::Translate(e) => {
                    (e.kind().to_string(), Some(e.span().start.offset))
                }
                _ => (err.to_string(), None),
            };
            let (pos, lineno, colno) = match (&pat, offset) {
                (Pattern::Text(s), Some(offset)) => {
                    let char_pos = error::byte_offset_to_char(s, offset);
                    let (l, c) = error::line_col_text(s, char_pos);
                    (Some(char_pos), Some(l), Some(c))
                }
                (Pattern::Bytes(b), Some(offset)) => {
                    let (l, c) = error::line_col_bytes(b, offset);
                    (Some(offset), Some(l), Some(c))
                }
                _ => (None, None, None),
            };
            return Err(error::make_error(py, &msg, pattern, pos, lineno, colno)?);
        }

        // Build the engine program. Errors here are engine limits
        // (e.g. compiled too big), which have no pattern position.
        let inner = if bytes_mode {
            let built = regex::bytes::RegexBuilder::new(text)
                .case_insensitive(cfg.case_insensitive)
                .multi_line(cfg.multi_line)
                .dot_matches_new_line(cfg.dot_matches_new_line)
                .ignore_whitespace(cfg.ignore_whitespace)
                .unicode(cfg.unicode)
                .build();
            let re = match built {
                Ok(re) => re,
                Err(e) => {
                    return Err(error::make_error(
                        py,
                        &e.to_string(),
                        pattern,
                        None,
                        None,
                        None,
                    )?);
                }
            };
            Inner::Bytes(Box::new(re))
        } else {
            let built = regex::RegexBuilder::new(text)
                .case_insensitive(cfg.case_insensitive)
                .multi_line(cfg.multi_line)
                .dot_matches_new_line(cfg.dot_matches_new_line)
                .ignore_whitespace(cfg.ignore_whitespace)
                .unicode(cfg.unicode)
                .build();
            let re = match built {
                Ok(re) => re,
                Err(e) => {
                    return Err(error::make_error(
                        py,
                        &e.to_string(),
                        pattern,
                        None,
                        None,
                        None,
                    )?);
                }
            };
            Inner::Text(Box::new(re))
        };
        Ok(Regex {
            inner,
            pattern: pat,
            flags: cfg.stored_flags,
        })
    }
}

#[pymethods]
impl Regex {
    #[new]
    #[pyo3(signature = (pattern, flags = 0))]
    fn new(pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Self> {
        Self::build(pattern, flags)
    }

    /// The pattern text this regex was compiled from.
    #[getter]
    fn pattern<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match &self.pattern {
            Pattern::Text(s) => PyString::new(py, s).into_any(),
            Pattern::Bytes(b) => PyBytes::new(py, b).into_any(),
        }
    }

    /// The flags value this regex was compiled with. Like `re`,
    /// `UNICODE` is reported for str patterns (it is forced on unless
    /// `ASCII` is given).
    #[getter]
    fn flags(&self) -> u32 {
        self.flags
    }

    /// The number of capture groups.
    #[getter]
    fn groups(&self) -> usize {
        match &self.inner {
            // captures_len() counts the implicit whole-match group.
            Inner::Text(re) => re.captures_len() - 1,
            Inner::Bytes(re) => re.captures_len() - 1,
        }
    }

    /// A mapping of group name to group number.
    #[getter]
    fn groupindex<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let dict = PyDict::new(py);
        match &self.inner {
            Inner::Text(re) => {
                for (i, name) in re.capture_names().enumerate() {
                    if let Some(name) = name {
                        let _ = dict.set_item(name, i);
                    }
                }
            }
            Inner::Bytes(re) => {
                for (i, name) in re.capture_names().enumerate() {
                    if let Some(name) = name {
                        let _ = dict.set_item(name, i);
                    }
                }
            }
        }
        dict
    }
}
