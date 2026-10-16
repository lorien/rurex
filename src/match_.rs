//! The `Match` type returned by the matching methods.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyString, PyTuple};

use crate::error;
use crate::regex::Regex;
use crate::template;

/// The `endpos` value reported by a `Match`.
///
/// `Eof` (the overwhelmingly common `endpos=None` case) is resolved
/// lazily: counting the characters of a large text is O(n) in the
/// text, `endpos` is rarely read, and paying that cost on every
/// match made `search` linear in the whole haystack even when the
/// match sat in the first bytes. The first read counts (bytes mode:
/// measures) and caches the value.
#[derive(Debug)]
pub(crate) enum Endpos {
    Given(usize),
    Eof(OnceLock<usize>),
}

impl Endpos {
    pub(crate) fn from_opt(endpos: Option<usize>) -> Endpos {
        match endpos {
            Some(e) => Endpos::Given(e),
            None => Endpos::Eof(OnceLock::new()),
        }
    }

    /// A fresh equivalent value for another `Match` (the lazy cache
    /// is per-match, so it is not shared).
    pub(crate) fn replicate(&self) -> Endpos {
        match self {
            Endpos::Given(n) => Endpos::Given(*n),
            Endpos::Eof(_) => Endpos::Eof(OnceLock::new()),
        }
    }
}

/// The result of a successful match: spans are pre-converted to Python
/// units (character offsets for `str`, byte offsets for `bytes`), so no
/// conversion work happens after construction.
#[pyclass(name = "Match", module = "rurex", frozen)]
pub struct Match {
    re: Py<Regex>,
    string: Py<PyAny>,
    pos: usize,
    endpos: Endpos,
    /// Group spans in Python units; index 0 is the whole match.
    spans_py: Vec<Option<(usize, usize)>>,
    /// Capture names by group index (index 0 is unnamed). Shared with
    /// the Regex and every other Match it produced: cloning is a
    /// refcount bump, not a deep copy.
    names: Arc<Vec<Option<String>>>,
    bytes_mode: bool,
}

impl Match {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        re: Py<Regex>,
        string: Py<PyAny>,
        pos: usize,
        endpos: Endpos,
        spans_py: Vec<Option<(usize, usize)>>,
        names: Arc<Vec<Option<String>>>,
        bytes_mode: bool,
    ) -> Self {
        Match {
            re,
            string,
            pos,
            endpos,
            spans_py,
            names,
            bytes_mode,
        }
    }

    /// Resolve a group argument (int or name) to a group index.
    fn resolve(&self, item: &Bound<'_, PyAny>) -> PyResult<usize> {
        if let Ok(n) = item.extract::<i64>() {
            if n < 0 || n as usize >= self.spans_py.len() {
                return Err(PyIndexError::new_err("no such group"));
            }
            Ok(n as usize)
        } else if let Ok(name) = item.extract::<&str>() {
            for (i, n) in self.names.iter().enumerate() {
                if n.as_deref() == Some(name) {
                    return Ok(i);
                }
            }
            Err(PyIndexError::new_err("no such group"))
        } else {
            Err(PyIndexError::new_err("no such group"))
        }
    }

    /// The matched text of one group, as a Python object.
    fn group_value<'py>(
        &self,
        py: Python<'py>,
        index: usize,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(span) = self.spans_py[index] else {
            return Ok(None);
        };
        // Python-level slicing of the stored string keeps lifetimes
        // simple; spans are already in the text's native units (chars
        // for str, bytes for bytes).
        let slice = pyo3::types::PySlice::new(py, span.0 as isize, span.1 as isize, 1);
        Ok(Some(self.string.bind(py).get_item(&slice)?))
    }
}

#[pymethods]
impl Match {
    /// The Regex object that produced this match.
    #[getter]
    fn re(&self, py: Python<'_>) -> Py<Regex> {
        self.re.clone_ref(py)
    }

    /// The text that was scanned (the original Python object).
    #[getter]
    fn string(&self, py: Python<'_>) -> Py<PyAny> {
        self.string.clone_ref(py)
    }

    /// The `pos` value passed to the matching method.
    #[getter]
    fn pos(&self) -> usize {
        self.pos
    }

    /// The `endpos` value passed to the matching method (or the text
    /// length when not given).
    #[getter]
    fn endpos(&self, py: Python<'_>) -> usize {
        match &self.endpos {
            Endpos::Given(n) => *n,
            Endpos::Eof(lock) => *lock.get_or_init(|| {
                let s = self.string.bind(py);
                if self.bytes_mode {
                    s.extract::<&[u8]>().map_or(0, <[u8]>::len)
                } else {
                    s.extract::<&str>().map_or(0, |s| s.chars().count())
                }
            }),
        }
    }

    /// The index of the last group to close, or None.
    #[getter]
    pub(crate) fn lastindex(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, span) in self.spans_py.iter().enumerate().skip(1) {
            let Some((_, end)) = span else { continue };
            best = match best {
                None => Some(i),
                Some(b) => {
                    let (_, bend) = self.spans_py[b].unwrap();
                    // Last to close: greatest end; ties to the lower
                    // group number (inner groups close first).
                    if end > &bend || (end == &bend && i < b) {
                        Some(i)
                    } else {
                        Some(b)
                    }
                }
            };
        }
        best
    }

    /// The name of the last group to close, or None.
    #[getter]
    fn lastgroup(&self) -> Option<String> {
        self.lastindex().and_then(|i| self.names[i].clone())
    }

    /// Legacy span table: a tuple of `(start, end)` per group, group
    /// 0 first; `(-1, -1)` for groups that did not participate. Kept
    /// for `re` compatibility (`SRE_Match.regs`).
    #[getter]
    fn regs(&self) -> Vec<(isize, isize)> {
        self.spans_py
            .iter()
            .map(|sp| sp.map_or((-1, -1), |(s, e)| (s as isize, e as isize)))
            .collect()
    }

    /// Return one or more subgroups of the match.
    #[pyo3(signature = (*args))]
    fn group<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if args.len() == 0 {
            return Ok(self.group_value(py, 0)?.expect("group 0 always matches"));
        }
        let mut values = Vec::with_capacity(args.len());
        for arg in args.iter() {
            let index = self.resolve(&arg)?;
            values.push(
                self.group_value(py, index)?
                    .unwrap_or_else(|| py.None().into_bound(py)),
            );
        }
        if args.len() == 1 {
            Ok(values.pop().expect("one value"))
        } else {
            Ok(PyTuple::new(py, values)?.into_any())
        }
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let index = self.resolve(item)?;
        Ok(self
            .group_value(py, index)?
            .unwrap_or_else(|| py.None().into_bound(py)))
    }

    /// All capturing groups; unmatched groups default to None.
    #[pyo3(signature = (default = None))]
    fn groups<'py>(
        &self,
        py: Python<'py>,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let mut values = Vec::with_capacity(self.spans_py.len() - 1);
        for i in 1..self.spans_py.len() {
            values.push(match self.group_value(py, i)? {
                Some(v) => v,
                None => default.clone().unwrap_or_else(|| py.None().into_bound(py)),
            });
        }
        PyTuple::new(py, values)
    }

    /// A dict of named groups; unmatched groups default to None.
    #[pyo3(signature = (default = None))]
    fn groupdict<'py>(
        &self,
        py: Python<'py>,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (i, name) in self.names.iter().enumerate() {
            if let Some(name) = name {
                let value = match self.group_value(py, i)? {
                    Some(v) => v,
                    None => default.clone().unwrap_or_else(|| py.None().into_bound(py)),
                };
                dict.set_item(name, value)?;
            }
        }
        Ok(dict)
    }

    /// Start offset of a group (-1 if it did not participate).
    #[pyo3(signature = (group = None))]
    fn start(&self, py: Python<'_>, group: Option<Py<PyAny>>) -> PyResult<isize> {
        let index = self.group_arg(py, group)?;
        Ok(self.spans_py[index].map_or(-1, |(s, _)| s as isize))
    }

    /// End offset of a group (-1 if it did not participate).
    #[pyo3(signature = (group = None))]
    fn end(&self, py: Python<'_>, group: Option<Py<PyAny>>) -> PyResult<isize> {
        let index = self.group_arg(py, group)?;
        Ok(self.spans_py[index].map_or(-1, |(_, e)| e as isize))
    }

    /// (start, end) offsets of a group ((-1, -1) if it did not
    /// participate).
    #[pyo3(signature = (group = None))]
    fn span(&self, py: Python<'_>, group: Option<Py<PyAny>>) -> PyResult<(isize, isize)> {
        let index = self.group_arg(py, group)?;
        Ok(self.spans_py[index].map_or((-1, -1), |(s, e)| (s as isize, e as isize)))
    }

    /// Expand an `re`-style template against this match.
    fn expand<'py>(
        &self,
        py: Python<'py>,
        template: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let template_bytes: &[u8] = if self.bytes_mode {
            template
                .cast::<PyBytes>()
                .map_err(|_| PyTypeError::new_err("cannot use a str template on a bytes match"))?
                .as_bytes()
        } else {
            template
                .cast::<PyString>()
                .map_err(|_| PyTypeError::new_err("cannot use a bytes template on a str match"))?
                .to_str()
                .map_err(|_| PyTypeError::new_err("invalid template"))?
                .as_bytes()
        };
        let names_map: HashMap<&str, usize> = template::names_map(&self.names);
        let parsed = template::parse(template_bytes, self.spans_py.len() - 1, &names_map)
            .map_err(|e| self.template_error(py, e, template, template_bytes))?;
        let mut out = Vec::new();
        for piece in &parsed.pieces {
            match piece {
                template::Piece::Lit(range) => {
                    out.extend_from_slice(&template_bytes[range.clone()])
                }
                template::Piece::Byte(b) => out.push(*b),
                template::Piece::Char(v) => {
                    if self.bytes_mode {
                        out.push(*v as u8);
                    } else {
                        let mut buf = [0u8; 4];
                        if let Some(ch) = char::from_u32(*v) {
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                }
                template::Piece::Group(index) => {
                    if let Some(value) = self.group_value(py, *index)? {
                        if self.bytes_mode {
                            out.extend_from_slice(
                                value.cast::<PyBytes>().expect("bytes piece").as_bytes(),
                            );
                        } else {
                            let s = value
                                .cast::<PyString>()
                                .expect("str piece")
                                .to_str()
                                .map_err(|_| PyTypeError::new_err("invalid piece"))?;
                            out.extend_from_slice(s.as_bytes());
                        }
                    }
                }
            }
        }
        Ok(if self.bytes_mode {
            PyBytes::new(py, &out).into_any()
        } else {
            PyString::new(py, &String::from_utf8_lossy(&out)).into_any()
        })
    }

    /// Typing subscription (`rurex.Match[str]`), like `re.Match`.
    #[classmethod]
    fn __class_getitem__<'py>(
        cls: &Bound<'py, pyo3::types::PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, pyo3::types::PyGenericAlias>> {
        pyo3::types::PyGenericAlias::new(cls.py(), cls.as_any(), item)
    }

    fn __copy__<'a>(slf: PyRef<'a, Self>) -> PyRef<'a, Self> {
        slf
    }

    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, _memo: &Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }

    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<String> {
        let (start, end) = self.spans_py[0].unwrap_or((0, 0));
        let piece = self.group_value(py, 0)?;
        let piece_repr = match piece {
            Some(p) => p.repr()?.to_string(),
            None => "None".to_string(),
        };
        Ok(format!(
            "<rurex.Match object; span=({}, {}), match={}>",
            start, end, piece_repr
        ))
    }
}

impl Match {
    /// Start of group 0 in Python units (internal use).
    pub(crate) fn start0(&self) -> isize {
        self.spans_py[0].map_or(-1, |(s, _)| s as isize)
    }

    /// Resolve an optional group argument: None means group 0.
    fn group_arg(&self, py: Python<'_>, group: Option<Py<PyAny>>) -> PyResult<usize> {
        match group {
            None => Ok(0),
            Some(g) => self.resolve(g.bind(py)),
        }
    }

    /// Raise a template error as `rurex.error` (or `IndexError` for the
    /// unknown-group-name case, mirroring CPython), with the template
    /// as `pattern` and char positions for text mode.
    fn template_error(
        &self,
        py: Python<'_>,
        e: template::TplError,
        template_obj: &Bound<'_, PyAny>,
        template_bytes: &[u8],
    ) -> PyErr {
        if e.index_error {
            return PyIndexError::new_err(e.msg);
        }
        let (pos, lineno, colno) = if self.bytes_mode {
            let (l, c) = error::line_col_bytes(template_bytes, e.pos);
            (Some(e.pos), Some(l), Some(c))
        } else {
            let text = String::from_utf8_lossy(template_bytes).into_owned();
            let p = error::byte_offset_to_char(&text, e.pos);
            let (l, c) = error::line_col_text(&text, p);
            (Some(p), Some(l), Some(c))
        };
        error::make_error(py, &e.msg, template_obj, pos, lineno, colno)
            .unwrap_or_else(|_| error::Error::new_err(e.msg))
    }
}
