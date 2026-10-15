//! The `Regex` type: a compile-once wrapper over the engine, plus the
//! lazy `FindIter` returned by `finditer`.

use std::collections::HashMap;
use std::sync::OnceLock;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyString, PyTuple};

use crate::error;
use crate::flags;
use crate::match_::Match;
use crate::template;

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
    /// Number of capture groups (excluding the implicit group 0).
    groups: usize,
    /// Lazily compiled `\A(?:pattern)\z` variant used by `fullmatch`.
    /// `None` means the variant could not be built and the span-check
    /// fallback is used instead.
    anchored: OnceLock<Option<Inner>>,
}

/// Convert engine byte spans to Python-unit spans (character offsets)
/// with a single walk over the unique offsets.
fn spans_to_py(s: &str, spans: &[Option<(usize, usize)>]) -> Vec<Option<(usize, usize)>> {
    let mut offsets = Vec::with_capacity(spans.len() * 2);
    for span in spans.iter().flatten() {
        offsets.push(span.0);
        offsets.push(span.1);
    }
    offsets.sort_unstable();
    offsets.dedup();
    let mut map = HashMap::with_capacity(offsets.len());
    let mut chars = 0usize;
    let mut prev = 0usize;
    for &off in &offsets {
        chars += s[prev..off].chars().count();
        prev = off;
        map.insert(off, chars);
    }
    spans
        .iter()
        .map(|sp| sp.map(|(a, b)| (map[&a], map[&b])))
        .collect()
}

fn char_to_byte(s: &str, pos: usize) -> usize {
    match s.char_indices().nth(pos) {
        Some((b, _)) => b,
        None => s.len(),
    }
}

fn next_boundary(s: &str, at: usize) -> usize {
    if at >= s.len() {
        s.len() + 1
    } else {
        at + s[at..].chars().next().map_or(1, char::len_utf8)
    }
}

/// Clamp a scan window: pos clamped into the text, end defaults to the
/// text end and never precedes pos.
fn window(pos: usize, end: Option<usize>, len: usize) -> (usize, usize) {
    let p = pos.min(len);
    let e = end.map_or(len, |e| e.min(len)).max(p);
    (p, e)
}

pub(crate) enum Hay<'a> {
    Text(&'a str),
    Bytes(&'a [u8]),
}

/// Pre-parse a pattern with regex-syntax (the engine's own parser)
/// under `cfg`, so syntax errors carry structured positions for full
/// `re.error` attribute parity. `pattern` becomes the error's
/// `pattern` attribute; `text` is the (UTF-8 validated) pattern text.
pub(crate) fn validate_pattern(
    py: Python<'_>,
    pattern: &Bound<'_, PyAny>,
    text: &str,
    cfg: &flags::Config,
    bytes_mode: bool,
) -> PyResult<()> {
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
            regex_syntax::Error::Parse(e) => (e.kind().to_string(), Some(e.span().start.offset)),
            regex_syntax::Error::Translate(e) => {
                (e.kind().to_string(), Some(e.span().start.offset))
            }
            _ => (err.to_string(), None),
        };
        let (pos, lineno, colno) = match offset {
            Some(off) if bytes_mode => {
                let (l, c) = error::line_col_bytes(text.as_bytes(), off);
                (Some(off), Some(l), Some(c))
            }
            Some(off) => {
                let p = error::byte_offset_to_char(text, off);
                let (l, c) = error::line_col_text(text, p);
                (Some(p), Some(l), Some(c))
            }
            None => (None, None, None),
        };
        return Err(error::make_error(py, &msg, pattern, pos, lineno, colno)?);
    }
    Ok(())
}

/// An owned handle to `self` from a `PyRef` receiver (used to populate
/// `Match.re`).
fn py_handle(slf: &PyRef<'_, Regex>, py: Python<'_>) -> Py<Regex> {
    slf.into_pyobject(py)
        .expect("infallible")
        .to_owned()
        .unbind()
}

pub(crate) fn extract_hay<'py>(
    bytes_mode: bool,
    obj: &'py Bound<'py, PyAny>,
) -> PyResult<Hay<'py>> {
    if bytes_mode {
        let bytes_bound: &Bound<'py, PyBytes> = obj.cast::<PyBytes>().map_err(|_| {
            PyTypeError::new_err("cannot use a bytes pattern on a string-like object")
        })?;
        Ok(Hay::Bytes(bytes_bound.as_bytes()))
    } else {
        Ok(Hay::Text(
            obj.cast::<PyString>()
                .map_err(|_| {
                    PyTypeError::new_err("cannot use a string pattern on a bytes-like object")
                })?
                .to_str()?,
        ))
    }
}

/// Capture names by group index (index 0 unnamed).
fn names_of(re: &regex::Regex) -> Vec<Option<String>> {
    re.capture_names().map(|n| n.map(str::to_string)).collect()
}

fn names_of_bytes(re: &regex::bytes::Regex) -> Vec<Option<String>> {
    re.capture_names().map(|n| n.map(str::to_string)).collect()
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
        let cfg = flags::translate(flags, bytes_mode).map_err(PyValueError::new_err)?;

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

        validate_pattern(py, pattern, text, &cfg, bytes_mode)?;

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
        let groups = match &inner {
            Inner::Text(re) => re.captures_len() - 1,
            Inner::Bytes(re) => re.captures_len() - 1,
        };
        Ok(Regex {
            inner,
            pattern: pat,
            flags: cfg.stored_flags,
            groups,
            anchored: OnceLock::new(),
        })
    }

    /// The lazily built `\A(?:pattern)\z` variant for `fullmatch`.
    fn anchored(&self) -> Option<&Inner> {
        self.anchored
            .get_or_init(|| {
                let bytes_mode = matches!(self.inner, Inner::Bytes(_));
                let cfg = flags::translate(self.flags, bytes_mode).ok()?;
                let text: &str = match &self.pattern {
                    Pattern::Text(s) => s,
                    Pattern::Bytes(b) => std::str::from_utf8(b).ok()?,
                };
                let wrapped = format!(r"\A(?:{})\z", text);
                if bytes_mode {
                    regex::bytes::RegexBuilder::new(&wrapped)
                        .case_insensitive(cfg.case_insensitive)
                        .multi_line(cfg.multi_line)
                        .dot_matches_new_line(cfg.dot_matches_new_line)
                        .ignore_whitespace(cfg.ignore_whitespace)
                        .unicode(cfg.unicode)
                        .build()
                        .ok()
                        .map(|re| Inner::Bytes(Box::new(re)))
                } else {
                    regex::RegexBuilder::new(&wrapped)
                        .case_insensitive(cfg.case_insensitive)
                        .multi_line(cfg.multi_line)
                        .dot_matches_new_line(cfg.dot_matches_new_line)
                        .ignore_whitespace(cfg.ignore_whitespace)
                        .unicode(cfg.unicode)
                        .build()
                        .ok()
                        .map(|re| Inner::Text(Box::new(re)))
                }
            })
            .as_ref()
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
        self.groups
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

    /// True if the pattern matches anywhere in `text`.
    fn is_match<'py>(&self, py: Python<'py>, text: &Bound<'py, PyAny>) -> PyResult<bool> {
        match extract_hay(matches!(self.inner, Inner::Bytes(_)), text)? {
            Hay::Text(s) => {
                let re = match &self.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                Ok(py.detach(|| re.is_match(s)))
            }
            Hay::Bytes(b) => {
                let re = match &self.inner {
                    Inner::Bytes(re) => re,
                    Inner::Text(_) => unreachable!(),
                };
                Ok(py.detach(|| re.is_match(b)))
            }
        }
    }

    /// First match anywhere in the `[pos, endpos)` window. Unlike
    /// slicing, `^`/`\A` still anchor at the real text start, exactly
    /// like `re.search`.
    #[pyo3(signature = (text, pos = 0, endpos = None))]
    fn search<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = py_handle(&slf, py);
        match extract_hay(bytes_mode, text)? {
            Hay::Text(s) => {
                let (pb, eb) = window(
                    char_to_byte(s, pos),
                    endpos.map(|e| char_to_byte(s, e)),
                    s.len(),
                );
                let engine = match &slf.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                let hay = &s[..eb];
                let spans = py.detach(|| captures_spans_text(engine, hay, pb));
                Ok(spans.map(|sp| {
                    let endpos_u = endpos.unwrap_or_else(|| s.chars().count());
                    Match::new(
                        re,
                        text.clone().unbind(),
                        pos,
                        endpos_u,
                        spans_to_py(s, &sp),
                        names_of(engine),
                        false,
                    )
                }))
            }
            Hay::Bytes(b) => {
                let (pb, eb) = window(pos, endpos, b.len());
                let engine = match &slf.inner {
                    Inner::Bytes(re) => re,
                    Inner::Text(_) => unreachable!(),
                };
                let hay = &b[..eb];
                let spans = py.detach(|| captures_spans_bytes(engine, hay, pb));
                Ok(spans.map(|sp| {
                    let endpos_u = endpos.unwrap_or(b.len());
                    Match::new(
                        re,
                        text.clone().unbind(),
                        pos,
                        endpos_u,
                        sp,
                        names_of_bytes(engine),
                        true,
                    )
                }))
            }
        }
    }

    /// Match anchored at the start of the `[pos, endpos)` window
    /// (`re.match` under its Python 3.15 name).
    #[pyo3(signature = (text, pos = 0, endpos = None))]
    fn prefixmatch<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        // Same scan as `search`, accepted only if the match starts
        // exactly at `pos` (leftmost-first guarantees the match at pos
        // is found first when it exists).
        let m = Self::search(slf, py, text, pos, endpos)?;
        Ok(m.filter(|m| m.start0() == pos as isize))
    }

    /// The whole `[pos, endpos)` window must match.
    #[pyo3(signature = (text, pos = 0, endpos = None))]
    fn fullmatch<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = py_handle(&slf, py);
        match extract_hay(bytes_mode, text)? {
            Hay::Text(s) => {
                let (pb, eb) = window(
                    char_to_byte(s, pos),
                    endpos.map(|e| char_to_byte(s, e)),
                    s.len(),
                );
                let hay = &s[pb..eb];
                match slf.anchored() {
                    Some(Inner::Text(engine)) => {
                        let spans = py.detach(|| captures_spans_text(engine, hay, 0));
                        Ok(spans.map(|sp| {
                            let sp = offset_spans(sp, pb);
                            let endpos_u = endpos.unwrap_or_else(|| s.chars().count());
                            Match::new(
                                re,
                                text.clone().unbind(),
                                pos,
                                endpos_u,
                                spans_to_py(s, &sp),
                                names_of(engine),
                                false,
                            )
                        }))
                    }
                    _ => {
                        // Fallback: plain search, require exact window span.
                        let engine = match &slf.inner {
                            Inner::Text(re) => re,
                            Inner::Bytes(_) => unreachable!(),
                        };
                        let whole = &s[..eb];
                        let spans = py.detach(|| captures_spans_text(engine, whole, pb));
                        Ok(spans.filter(|sp| sp[0] == Some((pb, eb))).map(|sp| {
                            let endpos_u = endpos.unwrap_or_else(|| s.chars().count());
                            Match::new(
                                re,
                                text.clone().unbind(),
                                pos,
                                endpos_u,
                                spans_to_py(s, &sp),
                                names_of(engine),
                                false,
                            )
                        }))
                    }
                }
            }
            Hay::Bytes(b) => {
                let (pb, eb) = window(pos, endpos, b.len());
                let hay = &b[pb..eb];
                match slf.anchored() {
                    Some(Inner::Bytes(engine)) => {
                        let spans = py.detach(|| captures_spans_bytes(engine, hay, 0));
                        Ok(spans.map(|sp| {
                            let sp = offset_spans(sp, pb);
                            let endpos_u = endpos.unwrap_or(b.len());
                            Match::new(
                                re,
                                text.clone().unbind(),
                                pos,
                                endpos_u,
                                sp,
                                names_of_bytes(engine),
                                true,
                            )
                        }))
                    }
                    _ => {
                        let engine = match &slf.inner {
                            Inner::Bytes(re) => re,
                            Inner::Text(_) => unreachable!(),
                        };
                        let whole = &b[..eb];
                        let spans = py.detach(|| captures_spans_bytes(engine, whole, pb));
                        Ok(spans.filter(|sp| sp[0] == Some((pb, eb))).map(|sp| {
                            let endpos_u = endpos.unwrap_or(b.len());
                            Match::new(
                                re,
                                text.clone().unbind(),
                                pos,
                                endpos_u,
                                sp,
                                names_of_bytes(engine),
                                true,
                            )
                        }))
                    }
                }
            }
        }
    }

    /// A lazy iterator of matches over the `[pos, endpos)` window.
    #[pyo3(signature = (text, pos = 0, endpos = None))]
    fn finditer<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<FindIter> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let (scan, end_b, endpos_u, names) = match extract_hay(bytes_mode, text)? {
            Hay::Text(s) => {
                let (pb, eb) = window(
                    char_to_byte(s, pos),
                    endpos.map(|e| char_to_byte(s, e)),
                    s.len(),
                );
                let engine = match &slf.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                (
                    pb,
                    eb,
                    endpos.unwrap_or_else(|| s.chars().count()),
                    names_of(engine),
                )
            }
            Hay::Bytes(b) => {
                let (pb, eb) = window(pos, endpos, b.len());
                let engine = match &slf.inner {
                    Inner::Bytes(re) => re,
                    Inner::Text(_) => unreachable!(),
                };
                (pb, eb, endpos.unwrap_or(b.len()), names_of_bytes(engine))
            }
        };
        Ok(FindIter {
            re: py_handle(&slf, py),
            string: text.clone().unbind(),
            pos,
            endpos: endpos_u,
            scan,
            end_b,
            chars_seen: 0,
            bytes_seen: 0,
            names,
            bytes_mode,
            done: false,
        })
    }

    /// All matches as values: whole-match strings without groups, the
    /// single group with one, tuples otherwise. Unmatched groups yield
    /// the empty string, like `re.findall`.
    #[pyo3(signature = (text, pos = 0, endpos = None))]
    fn findall<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut iter = Self::finditer(slf, py, text, pos, endpos)?;
        let mut items = Vec::new();
        while let Some(m) = iter.step(py)? {
            let value = if m.group_count() == 0 {
                m.piece(py, 0)?.expect("group 0 always matches")
            } else if m.group_count() == 1 {
                m.piece(py, 1)?
                    .unwrap_or_else(|| PyString::new(py, "").into_any())
            } else {
                let parts: Vec<Bound<'_, PyAny>> = (1..=m.group_count())
                    .map(|i| {
                        m.piece(py, i)
                            .map(|p| p.unwrap_or_else(|| PyString::new(py, "").into_any()))
                    })
                    .collect::<PyResult<_>>()?;
                PyTuple::new(py, parts)?.into_any()
            };
            items.push(value);
        }
        PyList::new(py, items)
    }

    /// Split `text` on matches, including capture groups in the result
    /// (None when a group did not participate).
    #[pyo3(signature = (text, maxsplit = 0))]
    fn split<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        text: &Bound<'py, PyAny>,
        maxsplit: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut items: Vec<Bound<'_, PyAny>> = Vec::new();
        match (extract_hay(bytes_mode, text)?, &slf.inner) {
            (Hay::Text(s), Inner::Text(engine)) => {
                let mut prev = 0usize;
                let mut scan = 0usize;
                let mut n = 0usize;
                loop {
                    if maxsplit != 0 && n == maxsplit {
                        break;
                    }
                    let Some(spans) = py.detach(|| captures_spans_text(engine, s, scan)) else {
                        break;
                    };
                    let (sb, se) = spans[0].expect("group 0 always matches");
                    items.push(PyString::new(py, &s[prev..sb]).into_any());
                    for span in spans.iter().skip(1) {
                        items.push(match span {
                            Some((a, b)) => PyString::new(py, &s[*a..*b]).into_any(),
                            None => py.None().into_bound(py),
                        });
                    }
                    prev = se;
                    scan = if sb == se { next_boundary(s, se) } else { se };
                    n += 1;
                }
                items.push(PyString::new(py, &s[prev..]).into_any());
            }
            (Hay::Bytes(b), Inner::Bytes(engine)) => {
                let mut prev = 0usize;
                let mut scan = 0usize;
                let mut n = 0usize;
                loop {
                    if maxsplit != 0 && n == maxsplit {
                        break;
                    }
                    let Some(spans) = py.detach(|| captures_spans_bytes(engine, b, scan)) else {
                        break;
                    };
                    let (sb, se) = spans[0].expect("group 0 always matches");
                    items.push(PyBytes::new(py, &b[prev..sb]).into_any());
                    for span in spans.iter().skip(1) {
                        items.push(match span {
                            Some((a, e)) => PyBytes::new(py, &b[*a..*e]).into_any(),
                            None => py.None().into_bound(py),
                        });
                    }
                    prev = se;
                    scan = if sb == se { se + 1 } else { se };
                    n += 1;
                }
                items.push(PyBytes::new(py, &b[prev..]).into_any());
            }
            _ => unreachable!(),
        }
        PyList::new(py, items)
    }

    /// Substitute matches: `repl` is an `re`-style template string or a
    /// callable receiving each `Match`.
    #[pyo3(signature = (repl, text, count = 0))]
    fn sub(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        repl: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
        count: usize,
    ) -> PyResult<Py<PyAny>> {
        let (result, _) = substitute(slf, py, repl, text, count)?;
        Ok(result)
    }

    /// Like `sub`, but returns `(result, number_of_substitutions)`.
    #[pyo3(signature = (repl, text, count = 0))]
    fn subn(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        repl: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
        count: usize,
    ) -> PyResult<(Py<PyAny>, usize)> {
        substitute(slf, py, repl, text, count)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let pattern_obj = self.pattern(py);
        let mut out = format!("rurex.compile({}", pattern_obj.repr()?);
        let default = if matches!(self.inner, Inner::Bytes(_)) {
            0
        } else {
            flags::UNICODE
        };
        if self.flags != default {
            let mut names = Vec::new();
            for (bit, name) in [
                (flags::IGNORECASE, "IGNORECASE"),
                (flags::MULTILINE, "MULTILINE"),
                (flags::DOTALL, "DOTALL"),
                (flags::VERBOSE, "VERBOSE"),
                (flags::ASCII, "ASCII"),
            ] {
                if self.flags & bit != 0 {
                    names.push(format!("rurex.{name}"));
                }
            }
            if names.is_empty() {
                out.push_str(&format!(", {}", self.flags));
            } else {
                out.push_str(&format!(", {}", names.join("|")));
            }
        }
        out.push(')');
        Ok(out)
    }

    /// Pickling support: recompile from pattern and flags.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Py<PyAny>, Bound<'py, PyAny>)> {
        let compile = py.import("rurex")?.getattr("compile")?.unbind();
        let args = PyTuple::new(
            py,
            [
                self.pattern(py),
                self.flags.into_pyobject(py).expect("infallible").into_any(),
            ],
        )?;
        Ok((compile, args.into_any()))
    }

    fn __copy__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __deepcopy__<'a>(slf: PyRef<'a, Self>, _memo: &Bound<'_, PyAny>) -> PyRef<'a, Self> {
        slf
    }
}

fn captures_spans_text(
    re: &regex::Regex,
    hay: &str,
    start: usize,
) -> Option<Vec<Option<(usize, usize)>>> {
    re.captures_at(hay, start).map(|caps| {
        (0..caps.len())
            .map(|i| caps.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn captures_spans_bytes(
    re: &regex::bytes::Regex,
    hay: &[u8],
    start: usize,
) -> Option<Vec<Option<(usize, usize)>>> {
    re.captures_at(hay, start).map(|caps| {
        (0..caps.len())
            .map(|i| caps.get(i).map(|m| (m.start(), m.end())))
            .collect()
    })
}

fn offset_spans(spans: Vec<Option<(usize, usize)>>, off: usize) -> Vec<Option<(usize, usize)>> {
    spans
        .into_iter()
        .map(|sp| sp.map(|(a, b)| (a + off, b + off)))
        .collect()
}

/// Shared implementation of `sub` and `subn`.
fn substitute(
    slf: PyRef<'_, Regex>,
    py: Python<'_>,
    repl: &Bound<'_, PyAny>,
    text: &Bound<'_, PyAny>,
    count: usize,
) -> PyResult<(Py<PyAny>, usize)> {
    let this: Py<Regex> = py_handle(&slf, py);
    let reg: &Regex = &slf;
    let bytes_mode = matches!(reg.inner, Inner::Bytes(_));
    // Validate the replacement eagerly (CPython parses templates even
    // when nothing matches).
    enum Repl {
        Template(Vec<u8>, template::Template),
        Callable(Py<PyAny>),
    }
    let names_vec: Vec<Option<String>> = match &reg.inner {
        Inner::Text(re) => names_of(re),
        Inner::Bytes(re) => names_of_bytes(re),
    };
    let names_map: HashMap<&str, usize> = template::names_map(&names_vec);
    let parsed = if repl.is_callable() {
        Repl::Callable(repl.clone().unbind())
    } else if bytes_mode {
        let bytes = repl
            .extract::<&[u8]>()
            .map_err(|_| PyTypeError::new_err("repl must be bytes or callable, not str"))?;
        Repl::Template(
            bytes.to_vec(),
            template::parse(bytes, slf.groups, &names_map)
                .map_err(|e| template_error(py, e, repl, bytes, true))?,
        )
    } else {
        let s = repl
            .extract::<&str>()
            .map_err(|_| PyTypeError::new_err("repl must be str or callable, not bytes"))?;
        Repl::Template(
            s.as_bytes().to_vec(),
            template::parse(s.as_bytes(), reg.groups, &names_map)
                .map_err(|e| template_error(py, e, repl, s.as_bytes(), false))?,
        )
    };

    match (extract_hay(bytes_mode, text)?, &reg.inner) {
        (Hay::Text(s), Inner::Text(engine)) => {
            let names = names_of(engine);
            let mut out = String::new();
            let mut prev = 0usize;
            let mut scan = 0usize;
            let mut n = 0usize;
            loop {
                if count != 0 && n == count {
                    break;
                }
                let Some(spans) = py.detach(|| captures_spans_text(engine, s, scan)) else {
                    break;
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                out.push_str(&s[prev..sb]);
                match &parsed {
                    Repl::Template(bytes, tpl) => {
                        let mut buf = Vec::new();
                        template::expand(tpl, bytes, &spans, s.as_bytes(), &mut buf);
                        // Template literals only split at ASCII
                        // boundaries, so this is lossless.
                        out.push_str(&String::from_utf8_lossy(&buf));
                    }
                    Repl::Callable(func) => {
                        let m = Match::new(
                            this.clone_ref(py),
                            text.clone().unbind(),
                            0,
                            s.chars().count(),
                            spans_to_py(s, &spans),
                            names.clone(),
                            false,
                        );
                        let result = func.bind(py).call1((m,))?;
                        let piece = result.extract::<&str>().map_err(|_| {
                            PyTypeError::new_err(format!(
                                "expected str instance, {} found",
                                result.get_type()
                            ))
                        })?;
                        out.push_str(piece);
                    }
                }
                prev = se;
                scan = if sb == se { next_boundary(s, se) } else { se };
                n += 1;
            }
            out.push_str(&s[prev..]);
            Ok((PyString::new(py, &out).into_any().unbind(), n))
        }
        (Hay::Bytes(b), Inner::Bytes(engine)) => {
            let names = names_of_bytes(engine);
            let mut out = Vec::new();
            let mut prev = 0usize;
            let mut scan = 0usize;
            let mut n = 0usize;
            loop {
                if count != 0 && n == count {
                    break;
                }
                let Some(spans) = py.detach(|| captures_spans_bytes(engine, b, scan)) else {
                    break;
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                out.extend_from_slice(&b[prev..sb]);
                match &parsed {
                    Repl::Template(bytes, tpl) => {
                        template::expand(tpl, bytes, &spans, b, &mut out);
                    }
                    Repl::Callable(func) => {
                        let m = Match::new(
                            this.clone_ref(py),
                            text.clone().unbind(),
                            0,
                            b.len(),
                            spans.clone(),
                            names.clone(),
                            true,
                        );
                        let result = func.bind(py).call1((m,))?;
                        let piece = result.extract::<&[u8]>().map_err(|_| {
                            PyTypeError::new_err(format!(
                                "expected bytes instance, {} found",
                                result.get_type()
                            ))
                        })?;
                        out.extend_from_slice(piece);
                    }
                }
                prev = se;
                scan = if sb == se { se + 1 } else { se };
                n += 1;
            }
            out.extend_from_slice(&b[prev..]);
            Ok((PyBytes::new(py, &out).into_any().unbind(), n))
        }
        _ => unreachable!(),
    }
}

fn template_error(
    py: Python<'_>,
    e: template::TplError,
    template_obj: &Bound<'_, PyAny>,
    template_bytes: &[u8],
    bytes_mode: bool,
) -> PyErr {
    if e.index_error {
        return pyo3::exceptions::PyIndexError::new_err(e.msg);
    }
    let (pos, lineno, colno) = if bytes_mode {
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

/// Lazy iterator over matches, returned by `Regex.finditer`.
#[pyclass(name = "FindIter", module = "rurex")]
pub struct FindIter {
    re: Py<Regex>,
    string: Py<PyAny>,
    pos: usize,
    endpos: usize,
    /// Byte scan position.
    scan: usize,
    /// Byte end bound.
    end_b: usize,
    /// Chars before `bytes_seen` (text mode only).
    chars_seen: usize,
    bytes_seen: usize,
    names: Vec<Option<String>>,
    bytes_mode: bool,
    done: bool,
}

#[pymethods]
impl FindIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Match>> {
        self.step(py)
    }
}

impl FindIter {
    /// Produce the next match, releasing the GIL around the scan.
    pub(crate) fn step(&mut self, py: Python<'_>) -> PyResult<Option<Match>> {
        if self.done || self.scan > self.end_b {
            self.done = true;
            return Ok(None);
        }
        let re_ref = self.re.bind(py).get();
        match (&re_ref.inner, self.bytes_mode) {
            (Inner::Text(engine), false) => {
                let s = self.string.bind(py).extract::<&str>()?;
                let hay = &s[..self.end_b];
                let scan = self.scan;
                let spans = py.detach(|| captures_spans_text(engine, hay, scan));
                let Some(spans) = spans else {
                    self.done = true;
                    return Ok(None);
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                let mut py_spans = Vec::with_capacity(spans.len());
                for span in &spans {
                    py_spans.push(span.map(|(a, b)| {
                        let base = self.chars_seen + s[self.bytes_seen..a].chars().count();
                        (base, base + s[a..b].chars().count())
                    }));
                }
                self.chars_seen += s[self.bytes_seen..se].chars().count();
                self.bytes_seen = se;
                self.scan = if sb == se { next_boundary(s, se) } else { se };
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos,
                    py_spans,
                    self.names.clone(),
                    false,
                )))
            }
            (Inner::Bytes(engine), true) => {
                let b = self.string.bind(py).extract::<&[u8]>()?;
                let hay = &b[..self.end_b];
                let scan = self.scan;
                let spans = py.detach(|| captures_spans_bytes(engine, hay, scan));
                let Some(spans) = spans else {
                    self.done = true;
                    return Ok(None);
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                self.scan = if sb == se { se + 1 } else { se };
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos,
                    spans.clone(),
                    self.names.clone(),
                    true,
                )))
            }
            _ => {
                self.done = true;
                Ok(None)
            }
        }
    }
}
