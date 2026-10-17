//! The `Regex` type: a compile-once wrapper over the engine, plus the
//! lazy `FindIter` returned by `finditer`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyGenericAlias;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyString, PyTuple, PyType};

use crate::error;
use crate::flags;
use crate::match_::{Endpos, Match};
use crate::retry::RetryEngine;
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
/// `rurex.Pattern(...)` directly). Mirrors `re.Pattern` (ADR-0008):
/// value equality on pattern text + flags, weakref-able, typing-
/// subscriptable, with a `scanner` for successive match/search.
#[pyclass(name = "Pattern", module = "rurex", frozen, weakref)]
pub struct Regex {
    inner: Inner,
    /// The pattern as the user gave it (`.pattern`, equality, repr).
    pattern: Pattern,
    /// The engine source after inline-flag adjustment (leading
    /// `(?a)` stripped, scoped `(?a:` rewritten to `(?-u:`).
    src: Pattern,
    flags: u32,
    /// Number of capture groups (excluding the implicit group 0).
    groups: usize,
    /// Capture names by group index (index 0 unnamed), shared with
    /// every `Match` this regex produces.
    names: Arc<Vec<Option<String>>>,
    /// Lazily compiled `\A(?:pattern)\z` variant used by `fullmatch`.
    /// `None` means the variant could not be built and the span-check
    /// fallback is used instead.
    anchored: OnceLock<Option<Inner>>,
    /// Lazily built must-advance engine for CPython >=3.7 zero-width
    /// iteration (see `cpython_step`); built on the first empty
    /// match ever encountered. `None` = not yet built or failed to
    /// build (callers then fall back to advancing one unit).
    retry: OnceLock<Option<Arc<RetryEngine>>>,
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

/// Byte bounds of the `[pos, endpos)` window in `s` (Python char
/// indices), from a single left-to-right walk that stops at the
/// farther of the two indices; `endpos=None` and `pos=0` walk nothing.
/// Unreached indices clamp to the text end, and `endpos < pos` yields
/// `None`, exactly like `window`.
fn window_bytes(s: &str, pos: usize, endpos: Option<usize>) -> Option<(usize, usize)> {
    let far = endpos.map_or(pos, |e| pos.max(e));
    let mut pb = if pos == 0 { Some(0) } else { None };
    let mut eb = s.len();
    if far > 0 {
        for (i, (b, _)) in s.char_indices().enumerate() {
            if i == pos {
                pb = Some(b);
            }
            if endpos == Some(i) {
                eb = b;
            }
            if i >= far {
                break;
            }
        }
    }
    window(pb.unwrap_or(s.len()), Some(eb), s.len())
}

fn next_boundary(s: &str, at: usize) -> usize {
    if at >= s.len() {
        s.len() + 1
    } else {
        at + s[at..].chars().next().map_or(1, char::len_utf8)
    }
}

/// One step of CPython's (>=3.7) zero-width iteration, shared by
/// every match-iteration site (`finditer`, `findall`, `split`, `sub`
/// and the scanners).
///
/// After an empty match at `p`, `re` retries at `p` requiring a
/// non-empty match, and yields that before moving on. The public
/// engine API cannot express that preference-order query, so the
/// retry runs on the lazily built must-advance engine (`retry.rs`);
/// when that is unavailable the old advance-one-unit rule applies.
///
/// `hay` is the windowed haystack (retry bound = its end), `scan`
/// the current unanchored search position, `pending` the must-advance
/// retry position carried across steps, `search` the ordinary
/// unanchored leftmost-first search, `advance` the one-unit step
/// (char boundary in text mode, `+1` in bytes mode).
fn cpython_step<S, A>(
    hay: &[u8],
    scan: usize,
    pending: &mut Option<usize>,
    search: S,
    retry: Option<&RetryEngine>,
    advance: A,
) -> Option<Vec<Option<(usize, usize)>>>
where
    S: FnOnce(usize) -> Option<Vec<Option<(usize, usize)>>>,
    A: FnOnce(usize) -> usize,
{
    if let Some(p) = *pending {
        *pending = None;
        if let Some(spans) = retry.and_then(|r| r.retry(hay, p, hay.len())) {
            return Some(spans);
        }
        return search(advance(p));
    }
    search(scan)
}

/// Apply the CPython (>=3.7) advance rule to a just-yielded match:
/// an empty match arms the must-advance retry at its start instead
/// of moving the scan position.
#[inline]
fn advance_after(spans: &[Option<(usize, usize)>], scan: &mut usize) -> Option<usize> {
    let (sb, se) = spans[0].expect("group 0 always matches");
    if sb == se {
        Some(sb)
    } else {
        *scan = se;
        None
    }
}

/// Clamp a scan window like CPython: pos and endpos are both clamped
/// into the text first, and endpos < pos then means no match at all.
fn window(pos: usize, end: Option<usize>, len: usize) -> Option<(usize, usize)> {
    let p = pos.min(len);
    let e = end.map_or(len, |e| e.min(len));
    (e >= p).then_some((p, e))
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
            regex_syntax::Error::Parse(e) => {
                match error::cppythonize_ast(e.kind(), e.span(), text) {
                    Some((msg, at)) => (msg, Some(at)),
                    None => (e.kind().to_string(), Some(e.span().start.offset)),
                }
            }
            regex_syntax::Error::Translate(e) => {
                match error::cppythonize_hir(e.kind(), e.span(), text) {
                    Some((msg, at)) => (msg, Some(at)),
                    None => (e.kind().to_string(), Some(e.span().start.offset)),
                }
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

/// Copy a bytes-like object (bytearray, memoryview, array, ...) to
/// fresh `bytes` via `builtins.bytes` — CPython's own buffer-protocol
/// conversion, so contiguity and type errors come out exactly as in
/// `re`. Exact `bytes` (and subclasses) never reach this helper; the
/// caller borrows them zero-copy instead.
pub(crate) fn bytes_like<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    // Reject non-buffers up front: `bytes(int)` would silently make
    // NUL bytes. `memoryview(obj)` succeeding is the authoritative
    // "supports the buffer protocol" check (and, unlike a raw buffer
    // export, it is available under the abi3 build).
    let mv = py.import("builtins")?.getattr("memoryview")?;
    mv.call1((obj,)).map_err(|_| {
        let type_name = obj
            .get_type()
            .name()
            .map_or_else(|_| "?".to_string(), |n| n.to_string());
        PyTypeError::new_err(format!(
            "expected string or bytes-like object, got '{}'",
            type_name
        ))
    })?;
    drop(mv);
    let converter = py.import("builtins")?.getattr("bytes")?;
    let owned = converter.call1((obj,)).map_err(|_| {
        let type_name = obj
            .get_type()
            .name()
            .map_or_else(|_| "?".to_string(), |n| n.to_string());
        PyTypeError::new_err(format!(
            "expected string or bytes-like object, got '{}'",
            type_name
        ))
    })?;
    owned
        .cast_into::<PyBytes>()
        .map_err(|_| PyTypeError::new_err("expected string or bytes-like object"))
}

/// Owns the haystack source for one call: the original `str`/`bytes`
/// object (borrowed zero-copy, subclasses included) or a `bytes` copy
/// of a bytes-like input (bytearray, memoryview, array, ...). `hay()`
/// derives the borrowed view, so call sites bind the `HayKeep` first.
pub(crate) enum HayKeep<'py> {
    Text(Bound<'py, PyString>),
    /// `copied` is true when `bound` is a snapshot of a bytes-like
    /// object rather than the input itself (iterators keep it).
    Bytes {
        bound: Bound<'py, PyBytes>,
        copied: bool,
    },
}

impl<'py> HayKeep<'py> {
    pub(crate) fn hay(&self) -> PyResult<Hay<'_>> {
        match self {
            HayKeep::Text(s) => Ok(Hay::Text(s.to_str()?)),
            HayKeep::Bytes { bound, .. } => Ok(Hay::Bytes(bound.as_bytes())),
        }
    }
}

pub(crate) fn extract_hay<'py>(
    bytes_mode: bool,
    obj: &'py Bound<'py, PyAny>,
) -> PyResult<HayKeep<'py>> {
    if bytes_mode {
        if let Ok(bound) = obj.cast::<PyBytes>() {
            return Ok(HayKeep::Bytes {
                bound: bound.clone(),
                copied: false,
            });
        }
        // CPython distinguishes a str haystack (specific message) from
        // any other wrong type (generic message).
        if obj.cast::<PyString>().is_ok() {
            return Err(PyTypeError::new_err(
                "cannot use a bytes pattern on a string-like object",
            ));
        }
        let owned = bytes_like(obj.py(), obj)?;
        Ok(HayKeep::Bytes {
            bound: owned,
            copied: true,
        })
    } else {
        let s = obj.cast::<PyString>().map_err(|_| {
            if obj.cast::<PyBytes>().is_ok() {
                PyTypeError::new_err("cannot use a string pattern on a bytes-like object")
            } else {
                let type_name = obj
                    .get_type()
                    .name()
                    .map_or_else(|_| "?".to_string(), |n| n.to_string());
                PyTypeError::new_err(format!(
                    "expected string or bytes-like object, got '{}'",
                    type_name
                ))
            }
        })?;
        Ok(HayKeep::Text(s.clone()))
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
        // Fold leading global inline flags into the flag bits,
        // rewrite scoped `(?a:` groups, and reject the two constructs
        // CPython rejects but the engine would accept (mid-pattern
        // global flags, stacked quantifiers). The user-visible
        // pattern text is unchanged (see `src`).
        let adjusted = |text: &str| -> PyResult<(String, u32)> {
            let (rewritten, extra) = flags::inline_adjust(text, flags & flags::VERBOSE != 0)
                .map_err(|at| {
                    let pos = error::byte_offset_to_char(text, at);
                    let (l, c) = error::line_col_text(text, pos);
                    error::make_error(
                        py,
                        "global flags not at the start of the expression",
                        pattern,
                        Some(pos),
                        Some(l),
                        Some(c),
                    )
                    .unwrap_or_else(|_| {
                        error::PatternError::new_err(
                            "global flags not at the start of the expression",
                        )
                    })
                })?;
            if let Some((msg, at)) = flags::group_name_check(&rewritten) {
                let pos = error::byte_offset_to_char(&rewritten, at);
                let (l, c) = error::line_col_text(&rewritten, pos);
                return Err(error::make_error(
                    py,
                    &msg,
                    pattern,
                    Some(pos),
                    Some(l),
                    Some(c),
                )?);
            }
            if let Some(at) = flags::multiple_repeat(&rewritten, extra & flags::VERBOSE != 0) {
                let pos = error::byte_offset_to_char(&rewritten, at);
                let (l, c) = error::line_col_text(&rewritten, pos);
                return Err(error::make_error(
                    py,
                    "multiple repeat",
                    pattern,
                    Some(pos),
                    Some(l),
                    Some(c),
                )?);
            }
            Ok((rewritten, extra))
        };
        let (src, extra) = match &pat {
            Pattern::Text(s) => {
                let (rewritten, extra) = adjusted(s)?;
                (Pattern::Text(rewritten), extra)
            }
            Pattern::Bytes(b) => {
                // Byte patterns must be valid UTF-8 (raw byte
                // literals are not supported; use \xNN escapes) —
                // checked before any rewriting.
                match std::str::from_utf8(b) {
                    Ok(s) => {
                        let (rewritten, extra) = adjusted(s)?;
                        (Pattern::Bytes(rewritten.into_bytes()), extra)
                    }
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
                }
            }
        };
        let flags = flags | extra;
        let cfg = flags::translate(flags, bytes_mode).map_err(PyValueError::new_err)?;

        // The engine and the validator see the adjusted source
        // (`src`), not the user-visible pattern.
        let text: &str = match &src {
            Pattern::Text(s) => s,
            Pattern::Bytes(b) => std::str::from_utf8(b).expect("checked above"),
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
        let names = Arc::new(match &inner {
            Inner::Text(re) => names_of(re),
            Inner::Bytes(re) => names_of_bytes(re),
        });
        Ok(Regex {
            inner,
            pattern: pat,
            src,
            flags: cfg.stored_flags,
            groups,
            names,
            anchored: OnceLock::new(),
            retry: OnceLock::new(),
        })
    }

    /// The lazily built must-advance engine (`retry.rs`), shared by
    /// every iteration site via `cpython_step`.
    fn retry_engine(&self) -> Option<&Arc<RetryEngine>> {
        self.retry
            .get_or_init(|| {
                let bytes_mode = matches!(self.inner, Inner::Bytes(_));
                let cfg = flags::translate(self.flags, bytes_mode).ok()?;
                let text: &str = match &self.src {
                    Pattern::Text(s) => s,
                    Pattern::Bytes(b) => std::str::from_utf8(b).ok()?,
                };
                RetryEngine::build(text, &cfg, bytes_mode).map(Arc::new)
            })
            .as_ref()
    }

    /// The lazily built `\A(?:pattern)\z` variant for `fullmatch`.
    fn anchored(&self) -> Option<&Inner> {
        self.anchored
            .get_or_init(|| {
                let bytes_mode = matches!(self.inner, Inner::Bytes(_));
                let cfg = flags::translate(self.flags, bytes_mode).ok()?;
                let text: &str = match &self.src {
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

    /// Shared `match`/`prefixmatch` implementation (kept out of
    /// `#[pymethods]` so it is not exposed as a Python method).
    pub(crate) fn match_impl<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        // Same scan as `search`, accepted only if the match starts
        // exactly at the (length-clamped) window start; leftmost-first
        // guarantees the match there is found first when it exists.
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        let start = match hay {
            // `pos == 0` skips the walk: counting every character of a
            // large text is O(n) and 0 is the answer anyway.
            Hay::Text(s) => {
                if pos == 0 {
                    0
                } else {
                    pos.min(s.chars().count())
                }
            }
            Hay::Bytes(b) => pos.min(b.len()),
        };
        let m = Self::search(slf, py, string, pos, endpos)?;
        Ok(m.filter(|m| m.start0() == start as isize))
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

    /// A mapping of group name to group number, read-only like
    /// `re.Pattern.groupindex` (a `types.MappingProxyType` view).
    #[getter]
    fn groupindex<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        let proxy = py.import("types")?.getattr("MappingProxyType")?;
        Ok(proxy.call1((dict,))?.into_any())
    }

    /// True if the pattern matches anywhere in `text`.
    fn is_match<'py>(&self, py: Python<'py>, string: &Bound<'py, PyAny>) -> PyResult<bool> {
        let keep = extract_hay(matches!(self.inner, Inner::Bytes(_)), string)?;
        let hay = keep.hay()?;
        match hay {
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
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn search<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = py_handle(&slf, py);
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        match hay {
            Hay::Text(s) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(None);
                };
                let engine = match &slf.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                let hay = &s[..eb];
                let spans = py.detach(|| captures_spans_text(engine, hay, pb));
                Ok(spans.map(|sp| {
                    Match::new(
                        re,
                        string.clone().unbind(),
                        pos,
                        Endpos::from_opt(endpos),
                        spans_to_py(s, &sp),
                        slf.names.clone(),
                        false,
                    )
                }))
            }
            Hay::Bytes(b) => {
                let Some((pb, eb)) = window(pos, endpos, b.len()) else {
                    return Ok(None);
                };
                let engine = match &slf.inner {
                    Inner::Bytes(re) => re,
                    Inner::Text(_) => unreachable!(),
                };
                let hay = &b[..eb];
                let spans = py.detach(|| captures_spans_bytes(engine, hay, pb));
                Ok(spans.map(|sp| {
                    Match::new(
                        re,
                        string.clone().unbind(),
                        pos,
                        Endpos::from_opt(endpos),
                        sp,
                        slf.names.clone(),
                        true,
                    )
                }))
            }
        }
    }

    /// Match anchored at the start of the `[pos, endpos)` window.
    /// The Python 3.15 preferred name (ADR-0007); `re.match` under its
    /// historic name — both expose the same behavior, like CPython.
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn prefixmatch<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        Self::match_impl(slf, py, string, pos, endpos)
    }

    /// The historic name for `prefixmatch`, soft deprecated in
    /// CPython 3.15 but kept for compatibility — exactly like `re`.
    #[pyo3(name = "match", signature = (string, pos = 0, endpos = None))]
    fn match_<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        Self::match_impl(slf, py, string, pos, endpos)
    }

    /// A scanner for successive `match`/`search` calls over one text,
    /// like `re.Pattern.scanner` (`_sre.SRE_Scanner`).
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn scanner<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<SreScanner> {
        // Same window computation as finditer; the scanner owns its
        // advancing cursor from there.
        let it = Self::finditer(slf, py, string, pos, endpos)?;
        Ok(SreScanner {
            re: it.re,
            string: it.string,
            snapshot: it.snapshot,
            pos: it.pos,
            endpos: it.endpos,
            scan: it.scan,
            pending: it.pending,
            end_b: it.end_b,
            chars_seen: it.chars_seen,
            bytes_seen: it.bytes_seen,
            names: it.names,
            bytes_mode: it.bytes_mode,
            done: it.done,
        })
    }

    /// The whole `[pos, endpos)` window must match.
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn fullmatch<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = py_handle(&slf, py);
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        match hay {
            Hay::Text(s) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(None);
                };
                let hay = &s[pb..eb];
                match slf.anchored() {
                    Some(Inner::Text(engine)) => {
                        let spans = py.detach(|| captures_spans_text(engine, hay, 0));
                        Ok(spans.map(|sp| {
                            let sp = offset_spans(sp, pb);
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                spans_to_py(s, &sp),
                                slf.names.clone(),
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
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                spans_to_py(s, &sp),
                                slf.names.clone(),
                                false,
                            )
                        }))
                    }
                }
            }
            Hay::Bytes(b) => {
                let Some((pb, eb)) = window(pos, endpos, b.len()) else {
                    return Ok(None);
                };
                let hay = &b[pb..eb];
                match slf.anchored() {
                    Some(Inner::Bytes(engine)) => {
                        let spans = py.detach(|| captures_spans_bytes(engine, hay, 0));
                        Ok(spans.map(|sp| {
                            let sp = offset_spans(sp, pb);
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                sp,
                                slf.names.clone(),
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
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                sp,
                                slf.names.clone(),
                                true,
                            )
                        }))
                    }
                }
            }
        }
    }

    /// A lazy iterator of matches over the `[pos, endpos)` window.
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn finditer<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<FindIter> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        // A bytes-like input (bytearray, memoryview, ...) is copied
        // once here: the iterator sees a snapshot, like `re` sees a
        // locked buffer (resize during iteration is invisible instead
        // of raising `BufferError`).
        let (window_ok, names, snapshot) = match (&hay, &keep) {
            (Hay::Text(s), _) => (window_bytes(s, pos, endpos), slf.names.clone(), None),
            (
                Hay::Bytes(b),
                HayKeep::Bytes {
                    bound,
                    copied: true,
                },
            ) => (
                window(pos, endpos, b.len()),
                slf.names.clone(),
                Some(bound.clone().unbind()),
            ),
            (Hay::Bytes(b), _) => (window(pos, endpos, b.len()), slf.names.clone(), None),
        };
        // An invalid window (endpos < pos after clamping) yields an
        // exhausted iterator immediately.
        let (scan, end_b, done) = match window_ok {
            Some((pb, eb)) => (pb, eb, false),
            None => (1, 0, true),
        };
        Ok(FindIter {
            re: py_handle(&slf, py),
            string: string.clone().unbind(),
            snapshot,
            pos,
            endpos: Endpos::from_opt(endpos),
            scan,
            pending: None,
            end_b,
            chars_seen: 0,
            bytes_seen: 0,
            names,
            bytes_mode,
            done,
        })
    }

    /// All matches as values: whole-match strings without groups, the
    /// single group with one, tuples otherwise. Unmatched groups yield
    /// the empty string, like `re.findall`.
    ///
    /// Dedicated path: all matches are scanned and their spans
    /// collected in one GIL-released Rust loop, then only the returned
    /// values are constructed — no `Match` objects, no per-match
    /// span-to-character conversion (byte-unit slices produce the same
    /// strings).
    #[pyo3(signature = (string, pos = 0, endpos = None))]
    fn findall<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Bound<'py, PyList>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        match (hay, &slf.inner) {
            (Hay::Text(s), Inner::Text(engine)) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(PyList::empty(py));
                };
                let retry = slf.retry_engine().map(Arc::as_ref);
                let all_spans = py.detach(|| scan_all_text(engine, retry, s, pb, eb));
                let mut items = Vec::with_capacity(all_spans.len());
                for spans in &all_spans {
                    items.push(findall_item_text(py, s, slf.groups, spans)?);
                }
                Ok(PyList::new(py, items)?)
            }
            (Hay::Bytes(b), Inner::Bytes(engine)) => {
                let Some((pb, eb)) = window(pos, endpos, b.len()) else {
                    return Ok(PyList::empty(py));
                };
                let retry = slf.retry_engine().map(Arc::as_ref);
                let all_spans = py.detach(|| scan_all_bytes(engine, retry, b, pb, eb));
                let mut items = Vec::with_capacity(all_spans.len());
                for spans in &all_spans {
                    items.push(findall_item_bytes(py, b, slf.groups, spans)?);
                }
                Ok(PyList::new(py, items)?)
            }
            _ => unreachable!(),
        }
    }

    /// Split `text` on matches, including capture groups in the result
    /// (None when a group did not participate).
    #[pyo3(signature = (string, maxsplit = 0))]
    fn split<'py>(
        slf: PyRef<'_, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        maxsplit: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut items: Vec<Bound<'_, PyAny>> = Vec::new();
        let keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay()?;
        match (hay, &slf.inner) {
            (Hay::Text(s), Inner::Text(engine)) => {
                let retry = slf.retry_engine().map(Arc::as_ref);
                let mut prev = 0usize;
                let mut scan = 0usize;
                let mut n = 0usize;
                let mut pending: Option<usize> = None;
                loop {
                    if maxsplit != 0 && n == maxsplit {
                        break;
                    }
                    let Some(spans) = py.detach(|| {
                        cpython_step(
                            s.as_bytes(),
                            scan,
                            &mut pending,
                            |at| captures_spans_text(engine, s, at),
                            retry,
                            |p| next_boundary(s, p),
                        )
                    }) else {
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
                    pending = advance_after(&spans, &mut scan).or(pending);
                    n += 1;
                }
                items.push(PyString::new(py, &s[prev..]).into_any());
            }
            (Hay::Bytes(b), Inner::Bytes(engine)) => {
                let retry = slf.retry_engine().map(Arc::as_ref);
                let mut prev = 0usize;
                let mut scan = 0usize;
                let mut n = 0usize;
                let mut pending: Option<usize> = None;
                loop {
                    if maxsplit != 0 && n == maxsplit {
                        break;
                    }
                    let Some(spans) = py.detach(|| {
                        cpython_step(
                            b,
                            scan,
                            &mut pending,
                            |at| captures_spans_bytes(engine, b, at),
                            retry,
                            |p| p + 1,
                        )
                    }) else {
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
                    pending = advance_after(&spans, &mut scan).or(pending);
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
    #[pyo3(signature = (repl, string, count = 0))]
    fn sub(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        repl: &Bound<'_, PyAny>,
        string: &Bound<'_, PyAny>,
        count: usize,
    ) -> PyResult<Py<PyAny>> {
        let (result, _) = substitute(slf, py, repl, string, count)?;
        Ok(result)
    }

    /// Like `sub`, but returns `(result, number_of_substitutions)`.
    #[pyo3(signature = (repl, string, count = 0))]
    fn subn(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        repl: &Bound<'_, PyAny>,
        string: &Bound<'_, PyAny>,
        count: usize,
    ) -> PyResult<(Py<PyAny>, usize)> {
        substitute(slf, py, repl, string, count)
    }

    /// Value equality like `re.Pattern`: same pattern text (str vs
    /// bytes distinguished) and same flags.
    fn __eq__<'py>(slf: PyRef<'_, Self>, other: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        let py = other.py();
        if let Ok(other) = other.extract::<PyRef<'_, Self>>() {
            let same_text = match (&slf.pattern, &other.pattern) {
                (Pattern::Text(a), Pattern::Text(b)) => a == b,
                (Pattern::Bytes(a), Pattern::Bytes(b)) => a == b,
                _ => false,
            };
            let eq = same_text && slf.flags == other.flags;
            Ok(eq
                .into_pyobject(py)
                .expect("infallible")
                .to_owned()
                .unbind()
                .into_any())
        } else {
            Ok(py.NotImplemented())
        }
    }

    /// Consistent with `__eq__`: hash of (mode, pattern text, flags).
    fn __hash__(&self) -> isize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        match &self.pattern {
            Pattern::Text(s) => {
                0u8.hash(&mut h);
                s.hash(&mut h);
            }
            Pattern::Bytes(b) => {
                1u8.hash(&mut h);
                b.hash(&mut h);
            }
        }
        self.flags.hash(&mut h);
        h.finish() as isize
    }

    /// Typing subscription (`rurex.Pattern[str]`), like `re.Pattern`.
    #[classmethod]
    fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyGenericAlias>> {
        PyGenericAlias::new(cls.py(), cls.as_any(), item)
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

/// Every match in the `[pb, eb)` byte window, as byte-unit group
/// spans — pure Rust, safe under a released GIL. Follows CPython's
/// (>=3.7) zero-width iteration rule via `cpython_step`.
fn scan_all_text(
    re: &regex::Regex,
    retry: Option<&RetryEngine>,
    s: &str,
    pb: usize,
    eb: usize,
) -> Vec<Vec<Option<(usize, usize)>>> {
    let hay = &s[..eb];
    let mut out = Vec::new();
    let mut scan = pb;
    let mut pending: Option<usize> = None;
    while scan <= eb {
        let Some(spans) = cpython_step(
            hay.as_bytes(),
            scan,
            &mut pending,
            |at| captures_spans_text(re, hay, at),
            retry,
            |p| next_boundary(s, p),
        ) else {
            break;
        };
        pending = advance_after(&spans, &mut scan).or(pending);
        out.push(spans);
    }
    out
}

fn scan_all_bytes(
    re: &regex::bytes::Regex,
    retry: Option<&RetryEngine>,
    b: &[u8],
    pb: usize,
    eb: usize,
) -> Vec<Vec<Option<(usize, usize)>>> {
    let hay = &b[..eb];
    let mut out = Vec::new();
    let mut scan = pb;
    let mut pending: Option<usize> = None;
    while scan <= eb {
        let Some(spans) = cpython_step(
            hay,
            scan,
            &mut pending,
            |at| captures_spans_bytes(re, hay, at),
            retry,
            |p| p + 1,
        ) else {
            break;
        };
        pending = advance_after(&spans, &mut scan).or(pending);
        out.push(spans);
    }
    out
}

/// One `findall` result value from byte-unit spans: the whole-match
/// string without groups, the single group's string with one, a tuple
/// of group strings otherwise (empty string for non-participating
/// groups, like `re.findall`).
fn findall_item_text<'py>(
    py: Python<'py>,
    s: &str,
    groups: usize,
    spans: &[Option<(usize, usize)>],
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match groups {
        0 => {
            let (a, b) = spans[0].expect("group 0 always matches");
            PyString::new(py, &s[a..b]).into_any()
        }
        1 => match spans[1] {
            Some((a, b)) => PyString::new(py, &s[a..b]).into_any(),
            None => PyString::new(py, "").into_any(),
        },
        _ => {
            let parts: Vec<Bound<'py, PyAny>> = spans[1..=groups]
                .iter()
                .map(|sp| match sp {
                    Some((a, b)) => PyString::new(py, &s[*a..*b]).into_any(),
                    None => PyString::new(py, "").into_any(),
                })
                .collect();
            PyTuple::new(py, parts)?.into_any()
        }
    })
}

fn findall_item_bytes<'py>(
    py: Python<'py>,
    b: &[u8],
    groups: usize,
    spans: &[Option<(usize, usize)>],
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match groups {
        0 => {
            let (a, e) = spans[0].expect("group 0 always matches");
            PyBytes::new(py, &b[a..e]).into_any()
        }
        1 => match spans[1] {
            Some((a, e)) => PyBytes::new(py, &b[a..e]).into_any(),
            None => PyBytes::new(py, b"").into_any(),
        },
        _ => {
            let parts: Vec<Bound<'py, PyAny>> = spans[1..=groups]
                .iter()
                .map(|sp| match sp {
                    Some((a, e)) => PyBytes::new(py, &b[*a..*e]).into_any(),
                    None => PyBytes::new(py, b"").into_any(),
                })
                .collect();
            PyTuple::new(py, parts)?.into_any()
        }
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
    string: &Bound<'_, PyAny>,
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
        // Bytes-like `repl` (bytearray, memoryview, ...) is accepted
        // like `re` — copied to plain bytes for template parsing.
        let repl_keep: Bound<'_, PyBytes> = if let Ok(b) = repl.cast::<PyBytes>() {
            b.clone()
        } else if repl.cast::<PyString>().is_ok() {
            return Err(PyTypeError::new_err(
                "repl must be bytes or callable, not str",
            ));
        } else {
            bytes_like(py, repl)
                .map_err(|_| PyTypeError::new_err("repl must be bytes or callable, not str"))?
        };
        let bytes = repl_keep.as_bytes();
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

    let keep = extract_hay(bytes_mode, string)?;
    let hay = keep.hay()?;
    match (hay, &reg.inner) {
        (Hay::Text(s), Inner::Text(engine)) => {
            let names = reg.names.clone();
            let retry = reg.retry_engine().map(Arc::as_ref);
            let mut out = String::new();
            let mut prev = 0usize;
            let mut scan = 0usize;
            let mut n = 0usize;
            let mut pending: Option<usize> = None;
            loop {
                if count != 0 && n == count {
                    break;
                }
                let Some(spans) = py.detach(|| {
                    cpython_step(
                        s.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, s, at),
                        retry,
                        |p| next_boundary(s, p),
                    )
                }) else {
                    break;
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                out.push_str(&s[prev..sb]);
                match &parsed {
                    Repl::Template(bytes, tpl) => {
                        let mut buf = Vec::new();
                        template::expand(tpl, bytes, &spans, s.as_bytes(), true, &mut buf);
                        // Template literals only split at ASCII
                        // boundaries, so this is lossless.
                        out.push_str(&String::from_utf8_lossy(&buf));
                    }
                    Repl::Callable(func) => {
                        let m = Match::new(
                            this.clone_ref(py),
                            string.clone().unbind(),
                            0,
                            Endpos::Eof(OnceLock::new()),
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
                pending = advance_after(&spans, &mut scan).or(pending);
                n += 1;
            }
            out.push_str(&s[prev..]);
            Ok((PyString::new(py, &out).into_any().unbind(), n))
        }
        (Hay::Bytes(b), Inner::Bytes(engine)) => {
            let names = reg.names.clone();
            let retry = reg.retry_engine().map(Arc::as_ref);
            let mut out = Vec::new();
            let mut prev = 0usize;
            let mut scan = 0usize;
            let mut n = 0usize;
            let mut pending: Option<usize> = None;
            loop {
                if count != 0 && n == count {
                    break;
                }
                let Some(spans) = py.detach(|| {
                    cpython_step(
                        b,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, b, at),
                        retry,
                        |p| p + 1,
                    )
                }) else {
                    break;
                };
                let (sb, se) = spans[0].expect("group 0 always matches");
                out.extend_from_slice(&b[prev..sb]);
                match &parsed {
                    Repl::Template(bytes, tpl) => {
                        template::expand(tpl, bytes, &spans, b, false, &mut out);
                    }
                    Repl::Callable(func) => {
                        let m = Match::new(
                            this.clone_ref(py),
                            string.clone().unbind(),
                            0,
                            Endpos::Eof(OnceLock::new()),
                            spans.clone(),
                            names.clone(),
                            true,
                        );
                        let result = func.bind(py).call1((m,))?;
                        // A bytes-like return (bytearray, ...) is
                        // accepted and copied, like `re`.
                        let piece_bound = result.cast::<PyBytes>().cloned().or_else(|_| {
                            bytes_like(py, &result).map_err(|_| {
                                PyTypeError::new_err(format!(
                                    "expected bytes instance, {} found",
                                    result.get_type()
                                ))
                            })
                        })?;
                        let piece: Vec<u8> = piece_bound.as_bytes().to_vec();
                        out.extend_from_slice(&piece);
                    }
                }
                prev = se;
                pending = advance_after(&spans, &mut scan).or(pending);
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
        .unwrap_or_else(|_| error::PatternError::new_err(e.msg))
}

/// Lazy iterator over matches, returned by `Regex.finditer`.
#[pyclass(name = "FindIter", module = "rurex")]
pub struct FindIter {
    re: Py<Regex>,
    string: Py<PyAny>,
    /// Snapshot of a bytes-like input (bytearray, ...); `string`
    /// stays the original object for `Match.string` parity.
    snapshot: Option<Py<PyBytes>>,
    pos: usize,
    endpos: Endpos,
    /// Byte scan position.
    scan: usize,
    /// Must-advance retry position (CPython >=3.7 zero-width rule;
    /// see `cpython_step`).
    pending: Option<usize>,
    /// Byte end bound.
    end_b: usize,
    /// Chars before `bytes_seen` (text mode only).
    chars_seen: usize,
    bytes_seen: usize,
    names: Arc<Vec<Option<String>>>,
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
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let spans = py.detach(|| {
                    cpython_step(
                        hay.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, hay, at),
                        retry,
                        |p| next_boundary(s, p),
                    )
                });
                let Some(spans) = spans else {
                    self.pending = pending;
                    self.done = true;
                    return Ok(None);
                };
                let (_sb, se) = spans[0].expect("group 0 always matches");
                let mut py_spans = Vec::with_capacity(spans.len());
                for span in &spans {
                    py_spans.push(span.map(|(a, b)| {
                        let base = self.chars_seen + s[self.bytes_seen..a].chars().count();
                        (base, base + s[a..b].chars().count())
                    }));
                }
                self.chars_seen += s[self.bytes_seen..se].chars().count();
                self.bytes_seen = se;
                self.pending = advance_after(&spans, &mut self.scan).or(pending);
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
                    py_spans,
                    self.names.clone(),
                    false,
                )))
            }
            (Inner::Bytes(engine), true) => {
                let snapshot_bound;
                let string_bound;
                let b: &[u8] = match &self.snapshot {
                    Some(snap) => {
                        snapshot_bound = snap.bind(py);
                        snapshot_bound.as_bytes()
                    }
                    None => {
                        string_bound = self.string.bind(py).cast::<PyBytes>()?;
                        string_bound.as_bytes()
                    }
                };
                let hay = &b[..self.end_b];
                let scan = self.scan;
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let spans = py.detach(|| {
                    cpython_step(
                        hay,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, hay, at),
                        retry,
                        |p| p + 1,
                    )
                });
                let Some(spans) = spans else {
                    self.pending = pending;
                    self.done = true;
                    return Ok(None);
                };
                self.pending = advance_after(&spans, &mut self.scan).or(pending);
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
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

/// Successive `match`/`search` over one text, like `re.Pattern`'s
/// `scanner` (`_sre.SRE_Scanner`). Owns its scan state (same shape as
/// `FindIter`); `match` accepts only a match starting exactly at the
/// cursor, and — matching CPython — a failed `match` still consumes
/// the rejected match.
#[pyclass(name = "SRE_Scanner", module = "rurex")]
pub struct SreScanner {
    re: Py<Regex>,
    string: Py<PyAny>,
    snapshot: Option<Py<PyBytes>>,
    pos: usize,
    endpos: Endpos,
    scan: usize,
    /// Must-advance retry position (CPython >=3.7 zero-width rule).
    pending: Option<usize>,
    end_b: usize,
    chars_seen: usize,
    bytes_seen: usize,
    names: Arc<Vec<Option<String>>>,
    bytes_mode: bool,
    done: bool,
}

impl SreScanner {
    /// Produce the next match; `anchored` requires it to start exactly
    /// at the cursor (byte offset `self.scan`), and a failed anchored
    /// call advances nothing.
    fn next(&mut self, py: Python<'_>, anchored: bool) -> PyResult<Option<Match>> {
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
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let spans = py.detach(|| {
                    cpython_step(
                        hay.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, hay, at),
                        retry,
                        |p| next_boundary(s, p),
                    )
                });
                let Some(spans) = spans else {
                    self.pending = pending;
                    self.done = true;
                    return Ok(None);
                };
                let (_sb, se) = spans[0].expect("group 0 always matches");
                // Character offset of the cursor before this step (for
                // the anchored filter). Like CPython's SRE_Scanner, a
                // failed anchored call still consumes the match it
                // rejected — the state always advances.
                let cursor = if anchored {
                    Some(self.chars_seen + s[self.bytes_seen..scan].chars().count())
                } else {
                    None
                };
                let mut py_spans = Vec::with_capacity(spans.len());
                for span in &spans {
                    py_spans.push(span.map(|(a, b)| {
                        let base = self.chars_seen + s[self.bytes_seen..a].chars().count();
                        (base, base + s[a..b].chars().count())
                    }));
                }
                self.chars_seen += s[self.bytes_seen..se].chars().count();
                self.bytes_seen = se;
                self.pending = advance_after(&spans, &mut self.scan).or(pending);
                let m = Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
                    py_spans,
                    self.names.clone(),
                    false,
                );
                Ok(match cursor {
                    Some(c) if m.start0() != c as isize => None,
                    _ => Some(m),
                })
            }
            (Inner::Bytes(engine), true) => {
                let snapshot_bound;
                let string_bound;
                let b: &[u8] = match &self.snapshot {
                    Some(snap) => {
                        snapshot_bound = snap.bind(py);
                        snapshot_bound.as_bytes()
                    }
                    None => {
                        string_bound = self.string.bind(py).cast::<PyBytes>()?;
                        string_bound.as_bytes()
                    }
                };
                let hay = &b[..self.end_b];
                let scan = self.scan;
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let spans = py.detach(|| {
                    cpython_step(
                        hay,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, hay, at),
                        retry,
                        |p| p + 1,
                    )
                });
                let Some(spans) = spans else {
                    self.pending = pending;
                    self.done = true;
                    return Ok(None);
                };
                let (sb, _se) = spans[0].expect("group 0 always matches");
                // SRE_Scanner parity: an anchored miss still consumes.
                let reject = anchored && sb != scan;
                self.pending = advance_after(&spans, &mut self.scan).or(pending);
                if reject {
                    return Ok(None);
                }
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
                    spans,
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

#[pymethods]
impl SreScanner {
    /// The pattern this scanner was created from.
    #[getter]
    fn pattern<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.re.clone_ref(py).into_bound(py).into_any()
    }

    /// Next match from the cursor (unanchored).
    fn search(&mut self, py: Python<'_>) -> PyResult<Option<Match>> {
        self.next(py, false)
    }

    /// Next match anchored at the cursor. Like `SRE_Scanner.match`,
    /// a miss consumes the match it rejected — the cursor always
    /// advances by one call.
    #[pyo3(name = "match")]
    fn match_(&mut self, py: Python<'_>) -> PyResult<Option<Match>> {
        self.next(py, true)
    }
}

/// `re.Scanner` (ADR-0009): tokenize a string with a lexicon of
/// `(pattern, action)` pairs. The phrases are combined into one
/// compound pattern `"(p1)|(p2)|..."`; `scan` walks anchored
/// `SRE_Scanner` matches and applies the action of the branch that
/// matched (`m.lastindex - 1`), exactly like `Lib/re/__init__.py` in
/// CPython 3.13: callable actions are invoked as
/// `action(scanner, token)` with the matched text, non-callable
/// actions are used as-is, `None` results are dropped, and a
/// zero-width match stops the scan.
#[pyclass(name = "Scanner", module = "rurex")]
pub struct Scanner {
    /// The compiled compound pattern (CPython exposes this as
    /// `Scanner.scanner`).
    scanner: Py<Regex>,
    /// One action per lexicon entry.
    actions: Vec<Py<PyAny>>,
    /// The lexicon as given (CPython exposes `Scanner.lexicon`).
    lexicon: Vec<Py<PyAny>>,
    /// The match of the most recent callable action (CPython sets
    /// `Scanner.match`); interior mutability so `scan` can take an
    /// immutable, reentrant `PyRef`.
    last_match: Arc<Mutex<Option<Py<Match>>>>,
}

#[pymethods]
impl Scanner {
    #[new]
    #[pyo3(signature = (lexicon, flags = 0))]
    fn new<'py>(py: Python<'py>, lexicon: &Bound<'py, PyList>, flags: u32) -> PyResult<Self> {
        let mut phrases: Vec<String> = Vec::with_capacity(lexicon.len());
        let mut actions: Vec<Py<PyAny>> = Vec::with_capacity(lexicon.len());
        let mut lexicon_items: Vec<Py<PyAny>> = Vec::with_capacity(lexicon.len());
        for entry in lexicon.iter() {
            let pair = entry.extract::<(&str, Bound<'py, PyAny>)>().map_err(|_| {
                PyTypeError::new_err("lexicon must be a sequence of (pattern, action) pairs")
            })?;
            phrases.push(format!("({})", pair.0));
            actions.push(pair.1.clone().unbind());
            lexicon_items.push(entry.clone().unbind());
        }
        let combined = phrases.join("|");
        let pattern_obj = PyString::new(py, &combined);
        let regex = Regex::build(&pattern_obj, flags)?;
        Ok(Scanner {
            scanner: Py::new(py, regex)?,
            actions,
            lexicon: lexicon_items,
            last_match: Arc::new(Mutex::new(None)),
        })
    }

    /// The compiled compound pattern.
    #[getter]
    fn scanner<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.scanner.clone_ref(py).into_bound(py).into_any()
    }

    /// The lexicon this scanner was built from.
    #[getter]
    fn lexicon<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        PyList::new(py, self.lexicon.iter().map(|i| i.bind(py).clone()))
            .expect("rebuilding a list")
            .into_any()
    }

    /// The `Match` of the most recent callable action, if any.
    #[getter]
    fn get_match<'py>(&self, py: Python<'py>) -> Option<Bound<'py, Match>> {
        self.last_match
            .lock()
            .expect("scanner lock")
            .as_ref()
            .map(|m| m.bind(py).clone())
    }

    fn scan<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyAny>)> {
        let s = string.extract::<&str>().map_err(|_| {
            PyTypeError::new_err("cannot scan a bytes-like object with a str scanner")
        })?;
        // Everything needed after `slf` is consumed by into_pyobject.
        let actions: Vec<Py<PyAny>> = slf.actions.iter().map(|a| a.clone_ref(py)).collect();
        let last_match_cell = slf.last_match.clone();
        let re_handle = slf.scanner.clone_ref(py);
        let self_obj: Bound<'py, Scanner> = slf.into_pyobject(py)?;
        let mut state = SreScanner {
            re: re_handle,
            snapshot: None,
            string: string.clone().unbind(),
            pos: 0,
            endpos: Endpos::Eof(OnceLock::new()),
            scan: 0,
            pending: None,
            end_b: s.len(),
            chars_seen: 0,
            bytes_seen: 0,
            names: Arc::new(Vec::new()),
            bytes_mode: false,
            done: false,
        };
        let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
        let mut i_char = 0usize;
        // Byte end of the last accepted match — the rest is computed
        // from here (CPython's `i`), not from the scanner cursor,
        // which a rejected `match()` may have advanced past.
        let mut i_byte = 0usize;
        loop {
            let byte_start = state.bytes_seen;
            let m = state.next(py, true)?;
            let Some(m) = m else { break };
            let j_char = state.chars_seen;
            if i_char == j_char {
                break;
            }
            i_char = j_char;
            i_byte = state.bytes_seen;
            let token = PyString::new(py, &s[byte_start..state.bytes_seen]);
            let Some(lastindex) = m.lastindex() else {
                break;
            };
            let action = actions
                .get(lastindex - 1)
                .ok_or_else(|| PyIndexError::new_err("no lexicon action for the match"))?
                .clone_ref(py)
                .into_bound(py);
            let value = if action.is_callable() {
                let m_obj: Py<Match> = m.into_pyobject(py).expect("infallible").unbind();
                *last_match_cell.lock().expect("scanner lock") = Some(m_obj);
                action.call1((self_obj.clone(), token))?.unbind()
            } else {
                action.into_any().unbind()
            };
            if !value.is(py.None()) {
                result.push(value.into_bound(py));
            }
        }
        let rest = PyString::new(py, &s[i_byte..]);
        Ok((PyList::new(py, result)?, rest.into_any()))
    }
}
