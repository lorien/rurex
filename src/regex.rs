//! The `Regex` type: a compile-once wrapper over the engine, plus the
//! lazy `FindIter` returned by `finditer`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::marker::Ungil;
use pyo3::prelude::*;
use pyo3::types::PyGenericAlias;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyString, PyTuple, PyType};

use crate::error;
use crate::flags;
use crate::match_::{Endpos, Match, Spans};
use crate::prefilter::{self, Prefilter};
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
    /// every `Match` this regex produces. `None` when no group is
    /// named: the table is then never built, and a `Match` never pays
    /// a refcount bump for it.
    names: Option<Arc<Vec<Option<String>>>>,
    /// Lazily compiled `\A(?:pattern)\z` variant used by `fullmatch`.
    /// `None` means the variant could not be built and the span-check
    /// fallback is used instead.
    anchored: OnceLock<Option<Inner>>,
    /// Lazily built must-advance engine for CPython >=3.7 zero-width
    /// iteration (see `cpython_step`); built on the first empty
    /// match ever encountered. `None` = not yet built or failed to
    /// build (callers then fall back to advancing one unit).
    retry: OnceLock<Option<Arc<RetryEngine>>>,
    /// Lazily derived first-byte prefilter, consulted only on the
    /// cold-transcode path (see `prefilter.rs` and `cold()`).
    first: OnceLock<Option<Prefilter>>,
    /// Whether a lone surrogate in a `str` haystack may be mapped into
    /// the private-use stand-in block instead of raising. False when
    /// the pattern could tell the stand-in from a surrogate (see
    /// `utf8::pattern_allows_stand_in`); bytes patterns never use it.
    stand_in: bool,
    /// The engine configuration the pattern was compiled with, kept for
    /// the lazily built variants (`anchored`, the retry engine, the
    /// prefilter) so they never re-run flag translation.
    cfg: flags::Config,
    /// Never-reused id of this pattern, the key of its per-thread
    /// `CaptureLocations` scratch (see `text_spans`/`bytes_spans`).
    id: u64,
}

/// Rewrite engine byte spans to Python character offsets in place.
///
/// `ascii` means the haystack's UTF-8 is all-ASCII, so byte offsets
/// already are character offsets; then this returns untouched. This
/// predates every returned `Match`, and the sorted-offset walk it
/// replaces (which also built a hash map per call) was a third of the
/// cost of `search` on a short string. The distinct offsets fit a
/// stack buffer for any pattern with up to 16 groups; only larger
/// group counts take a heap buffer.
///
/// `start` is the (byte, char) position the walk begins at, so the
/// per-match loops (`finditer`, the scanners, a callable `sub`) count
/// only the characters between the previous match and this one instead
/// of walking the whole haystack again on every step. Every span
/// offset must be at or after `start.0`.
/// Build a Python `str` from a UTF-8 slice, restoring lone surrogates.
///
/// The engine matches UTF-8, so a lone surrogate in a `str` haystack is
/// matched through a private-use stand-in (see `utf8::SURROGATE_BASE`).
/// Results must be the text the user gave us, not the stand-in, so any
/// slice that contains the stand-in's UTF-8 prefix is rebuilt from its
/// code points with the surrogates restored (`surrogatepass` keeps them
/// alive in the resulting `str`). Slices without a stand-in take the
/// plain `PyString::new` path; the scan is a `memmem` over the slice,
/// which is short for a match.
fn text_result<'py>(py: Python<'py>, s: &str, mapped: bool) -> PyResult<Bound<'py, PyString>> {
    if !mapped || memchr::memmem::find(s.as_bytes(), b"\xF3\xB0").is_none() {
        return Ok(PyString::new(py, s));
    }
    let mut units: Vec<u16> = Vec::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        if crate::utf8::in_stand_in_block(cp) {
            units.push((0xD800 + (cp - crate::utf8::SURROGATE_BASE)) as u16);
        } else if cp > 0xFFFF {
            let v = cp - 0x1_0000;
            units.push(0xD800 + (v >> 10) as u16);
            units.push(0xDC00 + (v & 0x3FF) as u16);
        } else {
            units.push(cp as u16);
        }
    }
    let mut buf: Vec<u8> = Vec::with_capacity(units.len() * 2);
    for unit in units {
        buf.extend_from_slice(&unit.to_ne_bytes());
    }
    let ptr = unsafe {
        pyo3::ffi::PyUnicode_DecodeUTF16(
            buf.as_ptr() as *const std::os::raw::c_char,
            buf.len() as pyo3::ffi::Py_ssize_t,
            c"surrogatepass".as_ptr(),
            std::ptr::null_mut(),
        )
    };
    if ptr.is_null() {
        return Err(PyErr::fetch(py));
    }
    unsafe { Bound::<PyAny>::from_owned_ptr(py, ptr) }
        .cast_into::<PyString>()
        .map_err(PyErr::from)
}

fn to_py_spans(s: &str, ascii: bool, start: (usize, usize), spans: &mut [Option<(usize, usize)>]) {
    if ascii {
        return;
    }
    const INLINE: usize = 32;
    let n = spans.iter().flatten().count() * 2;
    if n == 0 {
        return;
    }
    let mut stack_bytes = [0usize; INLINE];
    let mut stack_chars = [0usize; INLINE];
    let mut heap_bytes;
    let mut heap_chars;
    let (bytes, chars): (&mut [usize], &mut [usize]) = if n <= INLINE {
        (&mut stack_bytes[..n], &mut stack_chars[..n])
    } else {
        heap_bytes = vec![0usize; n];
        heap_chars = vec![0usize; n];
        (heap_bytes.as_mut_slice(), heap_chars.as_mut_slice())
    };
    let mut k = 0;
    for span in spans.iter().flatten() {
        bytes[k] = span.0;
        bytes[k + 1] = span.1;
        k += 2;
    }
    bytes.sort_unstable();
    let mut unique = 0;
    for i in 0..n {
        if unique == 0 || bytes[i] != bytes[unique - 1] {
            bytes[unique] = bytes[i];
            unique += 1;
        }
    }
    let mut count = start.1;
    let mut prev = start.0;
    for i in 0..unique {
        count += s[prev..bytes[i]].chars().count();
        prev = bytes[i];
        chars[i] = count;
    }
    for span in spans.iter_mut() {
        if let Some((a, b)) = *span {
            let start = bytes[..unique].binary_search(&a).expect("span start");
            let end = bytes[..unique].binary_search(&b).expect("span end");
            *span = Some((chars[start], chars[end]));
        }
    }
}

/// Byte bounds of the `[pos, endpos)` window in `s` (Python char
/// indices), from a single left-to-right walk that stops at the
/// farther of the two indices; `endpos=None` and `pos=0` walk nothing.
/// Unreached indices clamp to the text end, and `endpos < pos` yields
/// `None`, exactly like `window`.
fn window_bytes(s: &str, pos: usize, endpos: Option<usize>) -> Option<(usize, usize)> {
    let far = endpos.map_or(pos, |e| pos.max(e));
    let mut pb = if pos == 0 { Some(0) } else { None };
    // `endpos = 0` is the one bound the walk below cannot discover,
    // because the walk is skipped when nothing is farther than 0.
    let mut eb = if endpos == Some(0) { 0 } else { s.len() };
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

/// Release the GIL around a scan only when the haystack is large
/// enough that the parallel window pays for the GIL handoff
/// (~1-4 µs under contention, measured on the Telegram benchmark:
/// 1.65 s single-threaded vs 5.38 s across four when every tiny
/// scan detached). Below the threshold, detaching costs more than
/// the scan itself and convoys the lock across threads. 4 KB is
/// ~0.4 µs of scanning at the engine's ~11 GB/s; even a 100x-slower
/// pattern stays well under a millisecond GIL-held (and `re` never
/// releases the GIL during matching at any size).
const GIL_RELEASE_MIN_HAY: usize = 4 * 1024;

/// `py.detach(f)` when the scan window is large enough to be worth
/// releasing the GIL for; `f()` GIL-held otherwise.
fn maybe_detach<F, T>(py: Python<'_>, hay_len: usize, f: F) -> T
where
    F: Ungil + FnOnce() -> T,
    T: Ungil,
{
    if hay_len >= GIL_RELEASE_MIN_HAY {
        py.detach(f)
    } else {
        f()
    }
}

/// Run one step of a per-match loop with the GIL held, however large
/// the haystack is.
///
/// `FindIter`, the scanners, `split` and `sub` ask the engine for one
/// match at a time. A step's scan normally ends a few bytes past the
/// previous match, so the handoff cannot be paid back: an uncontended
/// detach/attach costs tens of nanoseconds per step on a 160 KB
/// document (measured), and under GIL contention a handoff convoys the
/// lock for microseconds, which is why ADR-0014 found per-scan
/// detaching catastrophic there. Whole-window scans (`search`, `match`,
/// `fullmatch`, `findall`) still pass through `maybe_detach` once,
/// where the scan really does cover the window and the GIL can be
/// released for its duration.
fn per_step<T>(f: impl FnOnce() -> T) -> T {
    f()
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
enum StepSource {
    Retry(Spans),
    Search(usize),
}

/// The zero-width state machine shared by every iteration site: either
/// the pending must-advance retry produced spans, or the next search
/// starts at a given byte offset.
fn step_source<A>(
    hay: &[u8],
    scan: usize,
    pending: &mut Option<usize>,
    retry: Option<&RetryEngine>,
    advance: A,
) -> StepSource
where
    A: FnOnce(usize) -> usize,
{
    if let Some(p) = *pending {
        *pending = None;
        if let Some(spans) = retry.and_then(|r| r.retry(hay, p, hay.len())) {
            return StepSource::Retry(spans);
        }
        return StepSource::Search(advance(p));
    }
    StepSource::Search(scan)
}

fn cpython_step<S, A>(
    hay: &[u8],
    scan: usize,
    pending: &mut Option<usize>,
    search: S,
    retry: Option<&RetryEngine>,
    advance: A,
) -> Option<Spans>
where
    S: FnOnce(usize) -> Option<Spans>,
    A: FnOnce(usize) -> usize,
{
    match step_source(hay, scan, pending, retry, advance) {
        StepSource::Retry(spans) => Some(spans),
        StepSource::Search(at) => search(at),
    }
}

/// Like `cpython_step`, but appends the match's group spans to a flat
/// buffer instead of returning a fresh `Vec` per match. This is the
/// `findall` path: one allocation for the whole scan, not one per
/// match.
fn cpython_step_flat<S, A>(
    hay: &[u8],
    scan: usize,
    pending: &mut Option<usize>,
    out: &mut Vec<Option<(usize, usize)>>,
    search: S,
    retry: Option<&RetryEngine>,
    advance: A,
) -> bool
where
    S: FnOnce(usize, &mut Vec<Option<(usize, usize)>>) -> bool,
    A: FnOnce(usize) -> usize,
{
    match step_source(hay, scan, pending, retry, advance) {
        StepSource::Retry(spans) => {
            out.extend(spans.iter().copied());
            true
        }
        StepSource::Search(at) => search(at, out),
    }
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
    /// The UTF-8 view, plus whether a lone surrogate was mapped to a
    /// stand-in (in which case text results are rebuilt from the
    /// original object).
    Text(&'a str, bool),
    Bytes(&'a [u8]),
}

/// Explain a rejected pattern: re-parse it with `regex-syntax` (the
/// engine's own parser) under `cfg`, so syntax errors carry structured
/// positions for full `re.error` attribute parity. Only the failure
/// path calls this — the engine parses the pattern itself when it
/// succeeds, so an accepted pattern is parsed once (ADR-0015).
/// `pattern` becomes the error's `pattern` attribute; `text` is the
/// (UTF-8 validated) pattern text.
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
    Text(Bound<'py, PyString>, Option<crate::utf8::Utf8Scratch>),
    /// `copied` is true when `bound` is a snapshot of a bytes-like
    /// object rather than the input itself (iterators keep it).
    Bytes {
        bound: Bound<'py, PyBytes>,
        copied: bool,
    },
}

impl<'py> HayKeep<'py> {
    /// The haystack view. str inputs go through the three-way UTF-8
    /// borrow (`crate::utf8`): ASCII strings and strings whose
    /// CPython UTF-8 cache is warm are borrowed zero-copy; cold
    /// non-ASCII strings are transcoded in Rust (several times
    /// faster than CPython's encoder) into `owned`. CPython's cache
    /// is never populated by us.
    pub(crate) fn hay(&mut self, stand_in: bool) -> PyResult<Hay<'_>> {
        Ok(self
            .hay_impl(None, stand_in)?
            .expect("without a prefilter a haystack always exists"))
    }

    /// Like `hay`, but consult the pattern's first-byte prefilter on the
    /// cold-transcode path. `cold` is resolved lazily, so the pattern is
    /// only parsed for it when a cold string actually reaches the test.
    /// `None` means the prefilter proved that no match can exist in this
    /// haystack, so the caller returns its no-match answer without
    /// transcoding or scanning.
    pub(crate) fn hay_with(
        &mut self,
        cold: Option<&prefilter::Cold<'_>>,
        stand_in: bool,
    ) -> PyResult<Option<Hay<'_>>> {
        self.hay_impl(cold, stand_in)
    }

    fn hay_impl(
        &mut self,
        cold: Option<&prefilter::Cold<'_>>,
        stand_in: bool,
    ) -> PyResult<Option<Hay<'_>>> {
        match self {
            HayKeep::Text(bound, owned) => {
                match crate::utf8::utf8_of_filtered(bound, cold, stand_in)? {
                    crate::utf8::Utf8Buf::Borrowed(s, mapped) => Ok(Some(Hay::Text(s, mapped))),
                    crate::utf8::Utf8Buf::Owned(s) => {
                        let mapped = s.mapped();
                        *owned = Some(s);
                        Ok(Some(Hay::Text(
                            owned.as_ref().expect("just stored").as_str(),
                            mapped,
                        )))
                    }
                    crate::utf8::Utf8Buf::NoMatch => Ok(None),
                }
            }
            HayKeep::Bytes { bound, .. } => Ok(Some(Hay::Bytes(bound.as_bytes()))),
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
        Ok(HayKeep::Text(s.clone(), None))
    }
}

/// Capture names by group index (index 0 unnamed), or `None` when no
/// group is named.
fn names_of<'a, I: Iterator<Item = Option<&'a str>>>(it: I) -> Option<Arc<Vec<Option<String>>>> {
    let names: Vec<Option<String>> = it.map(|n| n.map(str::to_string)).collect();
    names.iter().any(Option::is_some).then(|| Arc::new(names))
}

fn names_of_text(re: &regex::Regex) -> Option<Arc<Vec<Option<String>>>> {
    names_of(re.capture_names())
}

fn names_of_bytes(re: &regex::bytes::Regex) -> Option<Arc<Vec<Option<String>>>> {
    names_of(re.capture_names())
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

        // The engine and the error-path validator see the adjusted
        // source (`src`), not the user-visible pattern.
        let text: &str = match &src {
            Pattern::Text(s) => s,
            Pattern::Bytes(b) => std::str::from_utf8(b).expect("checked above"),
        };

        // Build the engine program first. It parses the pattern itself,
        // so parsing beforehand (as the validator does) made every
        // accepted pattern be parsed twice; instead the `regex-syntax`
        // pass runs only for a pattern the engine rejected. That cannot
        // widen what compiles: the validator and the engine share one
        // `regex-syntax` version and one builder configuration, so a
        // pattern the engine accepts was always accepted by the
        // validator as well. A rejection is either a syntax error, which
        // the validator turns into an `re`-shaped error with positions,
        // or an engine limit (compiled program too large), which has no
        // pattern position.
        let built: Result<Inner, regex::Error> = if bytes_mode {
            regex::bytes::RegexBuilder::new(text)
                .case_insensitive(cfg.case_insensitive)
                .multi_line(cfg.multi_line)
                .dot_matches_new_line(cfg.dot_matches_new_line)
                .ignore_whitespace(cfg.ignore_whitespace)
                .unicode(cfg.unicode)
                .build()
                .map(|re| Inner::Bytes(Box::new(re)))
        } else {
            regex::RegexBuilder::new(text)
                .case_insensitive(cfg.case_insensitive)
                .multi_line(cfg.multi_line)
                .dot_matches_new_line(cfg.dot_matches_new_line)
                .ignore_whitespace(cfg.ignore_whitespace)
                .unicode(cfg.unicode)
                .build()
                .map(|re| Inner::Text(Box::new(re)))
        };
        let inner = match built {
            Ok(inner) => inner,
            // Propagates the validator's positional error; returns
            // `Ok` when the pattern is fine and the engine hit a limit.
            Err(e) => {
                validate_pattern(py, pattern, text, &cfg, bytes_mode)?;
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
        let groups = match &inner {
            Inner::Text(re) => re.captures_len() - 1,
            Inner::Bytes(re) => re.captures_len() - 1,
        };
        let names = match &inner {
            Inner::Text(re) => names_of_text(re),
            Inner::Bytes(re) => names_of_bytes(re),
        };
        let stand_in = !bytes_mode && crate::utf8::pattern_allows_stand_in(text);
        Ok(Regex {
            inner,
            pattern: pat,
            src,
            flags: cfg.stored_flags,
            groups,
            names,
            anchored: OnceLock::new(),
            retry: OnceLock::new(),
            first: OnceLock::new(),
            stand_in,
            cfg,
            // Two ids per pattern: the engine, and its anchored variant.
            id: NEXT_ID.fetch_add(2, Ordering::Relaxed),
        })
    }

    /// The lazy resolver for the first-byte prefilter, consulted only on
    /// the cold-transcode path. `None` for bytes patterns: the engine's
    /// own prefilter already scans those.
    fn cold(&self) -> Option<prefilter::Cold<'_>> {
        if matches!(self.inner, Inner::Bytes(_)) {
            return None;
        }
        let text = match &self.src {
            Pattern::Text(s) => s.as_str(),
            Pattern::Bytes(b) => std::str::from_utf8(b).ok()?,
        };
        Some(prefilter::Cold::new(&self.first, text, &self.cfg))
    }

    /// The lazily built must-advance engine (`retry.rs`), shared by
    /// every iteration site via `cpython_step`. Built on the first empty
    /// match ever seen: a pattern that cannot match empty never produces
    /// one, so it never pays for this engine.
    fn retry_engine(&self) -> Option<&Arc<RetryEngine>> {
        self.retry
            .get_or_init(|| {
                let bytes_mode = matches!(self.inner, Inner::Bytes(_));
                let text: &str = match &self.src {
                    Pattern::Text(s) => s,
                    Pattern::Bytes(b) => std::str::from_utf8(b).ok()?,
                };
                RetryEngine::build(text, &self.cfg, bytes_mode).map(Arc::new)
            })
            .as_ref()
    }

    /// The lazily built `\A(?:pattern)\z` variant for `fullmatch`.
    fn anchored(&self) -> Option<&Inner> {
        self.anchored
            .get_or_init(|| {
                let bytes_mode = matches!(self.inner, Inner::Bytes(_));
                let cfg = &self.cfg;
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
    pub(crate) fn match_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let slf = this.bind(py).get();
        // Same scan as `search`, accepted only if the match starts
        // exactly at the (length-clamped) window start; leftmost-first
        // guarantees the match there is found first when it exists.
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut keep = extract_hay(bytes_mode, string)?;
        let Some(hay) = keep.hay_with(slf.cold().as_ref(), slf.stand_in)? else {
            return Ok(None);
        };
        let start = match hay {
            // `pos == 0` skips the walk: counting every character of a
            // large text is O(n) and 0 is the answer anyway.
            Hay::Text(s, _mapped) => {
                if pos == 0 {
                    0
                } else {
                    pos.min(s.chars().count())
                }
            }
            Hay::Bytes(b) => pos.min(b.len()),
        };
        let m = Self::search_from(this, py, string, pos, endpos)?;
        Ok(m.filter(|m| m.start0() == start as isize))
    }

    pub(crate) fn fullmatch_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let slf = this.bind(py).get();
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = this.clone_ref(py);
        let mut keep = extract_hay(bytes_mode, string)?;
        let Some(hay) = keep.hay_with(slf.cold().as_ref(), slf.stand_in)? else {
            return Ok(None);
        };
        match hay {
            Hay::Text(s, _mapped) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(None);
                };
                let hay = &s[pb..eb];
                match slf.anchored() {
                    Some(Inner::Text(engine)) => {
                        let spans = maybe_detach(py, hay.len(), || {
                            captures_spans_text(engine, slf.id + 1, hay, 0)
                        });
                        Ok(spans.map(|mut sp| {
                            for span in sp.iter_mut() {
                                *span = span.map(|(a, b)| (a + pb, b + pb));
                            }
                            to_py_spans(s, crate::utf8::is_ascii_obj(string), (0, 0), &mut sp);
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                sp,
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
                        let spans = maybe_detach(py, whole.len(), || {
                            captures_spans_text(engine, slf.id, whole, pb)
                        });
                        Ok(spans.filter(|sp| sp[0] == Some((pb, eb))).map(|mut sp| {
                            to_py_spans(s, crate::utf8::is_ascii_obj(string), (0, 0), &mut sp);
                            Match::new(
                                re,
                                string.clone().unbind(),
                                pos,
                                Endpos::from_opt(endpos),
                                sp,
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
                        let spans = maybe_detach(py, hay.len(), || {
                            captures_spans_bytes(engine, slf.id + 1, hay, 0)
                        });
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
                        let spans = maybe_detach(py, whole.len(), || {
                            captures_spans_bytes(engine, slf.id, whole, pb)
                        });
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
    pub(crate) fn finditer_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<FindIter> {
        let slf = this.bind(py).get();
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut keep = extract_hay(bytes_mode, string)?;
        // A bytes-like input (bytearray, memoryview, ...) is copied
        // once here: the iterator sees a snapshot, like `re` sees a
        // locked buffer (resize during iteration is invisible instead
        // of raising `BufferError`). The snapshot is taken before the
        // mutable hay() borrow.
        let snapshot = match &keep {
            HayKeep::Bytes {
                bound,
                copied: true,
            } => Some(bound.clone().unbind()),
            _ => None,
        };
        // `None` from the prefilter means no match can exist, which is
        // the same state as an invalid window: an exhausted iterator.
        let window_ok = match keep.hay_with(slf.cold().as_ref(), slf.stand_in)? {
            Some(Hay::Text(s, _)) => window_bytes(s, pos, endpos),
            Some(Hay::Bytes(b)) => window(pos, endpos, b.len()),
            None => None,
        };
        let names = slf.names.clone();
        // An invalid window (endpos < pos after clamping) yields an
        // exhausted iterator immediately.
        let (scan, end_b, done) = match window_ok {
            Some((pb, eb)) => (pb, eb, false),
            None => (1, 0, true),
        };
        Ok(FindIter {
            re: this.clone_ref(py),
            string: string.clone().unbind(),
            snapshot,
            pos,
            endpos: Endpos::from_opt(endpos),
            scan,
            pending: None,
            end_b,
            chars_seen: 0,
            bytes_seen: 0,
            utf8: None,
            mapped: false,
            names,
            id: slf.id,
            bytes_mode,
            done,
        })
    }
    pub(crate) fn split_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        maxsplit: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let slf = this.bind(py).get();
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut items: Vec<Bound<'_, PyAny>> = Vec::new();
        let mut keep = extract_hay(bytes_mode, string)?;
        let hay = keep.hay(slf.stand_in)?;
        match (hay, &slf.inner) {
            (Hay::Text(s, mapped), Inner::Text(engine)) => {
                let retry = slf.retry_engine().map(Arc::as_ref);
                let mut prev = 0usize;
                let mut scan = 0usize;
                let mut n = 0usize;
                let mut pending: Option<usize> = None;
                loop {
                    if maxsplit != 0 && n == maxsplit {
                        break;
                    }
                    let Some(spans) = per_step(|| {
                        cpython_step(
                            s.as_bytes(),
                            scan,
                            &mut pending,
                            |at| captures_spans_text(engine, slf.id, s, at),
                            retry,
                            |p| next_boundary(s, p),
                        )
                    }) else {
                        break;
                    };
                    let (sb, se) = spans[0].expect("group 0 always matches");
                    items.push(text_result(py, &s[prev..sb], mapped)?.into_any());
                    for span in spans.iter().skip(1) {
                        items.push(match span {
                            Some((a, b)) => text_result(py, &s[*a..*b], mapped)?.into_any(),
                            None => py.None().into_bound(py),
                        });
                    }
                    prev = se;
                    pending = advance_after(&spans, &mut scan).or(pending);
                    n += 1;
                }
                items.push(text_result(py, &s[prev..], mapped)?.into_any());
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
                    let Some(spans) = per_step(|| {
                        cpython_step(
                            b,
                            scan,
                            &mut pending,
                            |at| captures_spans_bytes(engine, slf.id, b, at),
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
}

impl Regex {
    pub(crate) fn search_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Option<Match>> {
        let slf = this.bind(py).get();
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let re = this.clone_ref(py);
        let mut keep = extract_hay(bytes_mode, string)?;
        let Some(hay) = keep.hay_with(slf.cold().as_ref(), slf.stand_in)? else {
            return Ok(None);
        };
        match hay {
            Hay::Text(s, _mapped) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(None);
                };
                let engine = match &slf.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                let hay = &s[..eb];
                let spans = maybe_detach(py, hay.len(), || {
                    captures_spans_text(engine, slf.id, hay, pb)
                });
                Ok(spans.map(|mut sp| {
                    to_py_spans(s, crate::utf8::is_ascii_obj(string), (0, 0), &mut sp);
                    Match::new(
                        re,
                        string.clone().unbind(),
                        pos,
                        Endpos::from_opt(endpos),
                        sp,
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
                let spans = maybe_detach(py, hay.len(), || {
                    captures_spans_bytes(engine, slf.id, hay, pb)
                });
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

    pub(crate) fn findall_from<'py>(
        this: &Py<Regex>,
        py: Python<'py>,
        string: &Bound<'py, PyAny>,
        pos: usize,
        endpos: Option<usize>,
    ) -> PyResult<Bound<'py, PyList>> {
        let slf = this.bind(py).get();
        let bytes_mode = matches!(slf.inner, Inner::Bytes(_));
        let mut keep = extract_hay(bytes_mode, string)?;
        let Some(hay) = keep.hay_with(slf.cold().as_ref(), slf.stand_in)? else {
            return Ok(PyList::empty(py));
        };
        match (hay, &slf.inner) {
            (Hay::Text(s, mapped), Inner::Text(engine)) => {
                let Some((pb, eb)) = window_bytes(s, pos, endpos) else {
                    return Ok(PyList::empty(py));
                };
                let retry = slf.retry_engine().map(Arc::as_ref);
                let groups = slf.groups;
                let spans = maybe_detach(py, eb - pb, || {
                    scan_all_flat_text(engine, slf.id, retry, s, pb, eb, groups)
                });
                findall_list_text(py, s, groups, &spans, mapped)
            }
            (Hay::Bytes(b), Inner::Bytes(engine)) => {
                let Some((pb, eb)) = window(pos, endpos, b.len()) else {
                    return Ok(PyList::empty(py));
                };
                let retry = slf.retry_engine().map(Arc::as_ref);
                let groups = slf.groups;
                let spans = maybe_detach(py, eb - pb, || {
                    scan_all_flat_bytes(engine, slf.id, retry, b, pb, eb, groups)
                });
                findall_list_bytes(py, b, groups, &spans)
            }
            _ => unreachable!(),
        }
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
        let mut keep = extract_hay(matches!(self.inner, Inner::Bytes(_)), string)?;
        let Some(hay) = keep.hay_with(self.cold().as_ref(), self.stand_in)? else {
            return Ok(false);
        };
        match hay {
            Hay::Text(s, _mapped) => {
                let re = match &self.inner {
                    Inner::Text(re) => re,
                    Inner::Bytes(_) => unreachable!(),
                };
                let id = self.id;
                Ok(maybe_detach(py, s.len(), || {
                    with_text_engine(re, id, |engine, _| engine.is_match(s))
                }))
            }
            Hay::Bytes(b) => {
                let re = match &self.inner {
                    Inner::Bytes(re) => re,
                    Inner::Text(_) => unreachable!(),
                };
                let id = self.id;
                Ok(maybe_detach(py, b.len(), || {
                    with_bytes_engine(re, id, |engine, _| engine.is_match(b))
                }))
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
        let this = py_handle(&slf, py);
        Self::search_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        Self::match_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        Self::match_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        let it = Self::finditer_from(&this, py, string, pos, endpos)?;
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
            utf8: it.utf8,
            mapped: it.mapped,
            names: it.names,
            id: it.id,
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
        let this = py_handle(&slf, py);
        Self::fullmatch_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        Self::finditer_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        Self::findall_from(&this, py, string, pos, endpos)
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
        let this = py_handle(&slf, py);
        Self::split_from(&this, py, string, maxsplit)
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
        let this = py_handle(&slf, py);
        let (result, _) = substitute(&this, py, repl, string, count)?;
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
        let this = py_handle(&slf, py);
        substitute(&this, py, repl, string, count)
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

/// Engine ids are never reused, so a thread's cache slot for a freed
/// pattern can never be mistaken for a live one. Two ids are reserved
/// per pattern: the engine, and its lazily built anchored variant
/// (`fullmatch`).
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// How many patterns a thread keeps an engine and capture-location
/// scratch for. More than one so alternating patterns do not evict each
/// other every call, small enough that the storage stays bounded.
const CACHE_SLOTS: usize = 4;

/// Per-thread, per-pattern engine handle.
///
/// Two costs motivate it, both per call on the result path:
///
/// - `Regex::captures_at` allocates a `Captures` and its location
///   vector; `captures_read_at` writes into a caller-provided
///   `CaptureLocations`, so one set per pattern per thread makes the
///   call allocation-free (ADR-0017).
/// - A `regex::Regex` shared across threads serves every thread but its
///   owner through its internal cache pool, and a mutex on the search
///   path is what stops short-haystack scans from scaling: measured on
///   200k Telegram posts, a shared engine costs 134 ns per `is_match`
///   at four free-threaded workers against 89 ns for a per-thread clone
///   (ADR-0019). `Clone` deliberately drops the `Arc` around the pool,
///   so each clone keeps the shared immutable program and gets its own
///   caches.
///
/// The scratch belongs to the thread rather than to the pattern because
/// the pattern is shared. Entries are keyed by a never-reused pattern
/// id, so a slot left behind by a dropped pattern is never hit again; it
/// is simply overwritten when the next pattern misses.
struct Cache<E, L> {
    slots: [Slot<E, L>; CACHE_SLOTS],
    next: usize,
}

struct Slot<E, L> {
    /// 0 = never used; pattern ids start at 1.
    id: u64,
    engine: Option<E>,
    locs: Option<L>,
}

impl<E, L> Cache<E, L> {
    fn new() -> Self {
        Cache {
            slots: std::array::from_fn(|_| Slot {
                id: 0,
                engine: None,
                locs: None,
            }),
            next: 0,
        }
    }

    /// This thread's engine and scratch for pattern `id`, built by
    /// `make` on a miss. A miss takes the next slot round-robin.
    #[inline]
    fn get(&mut self, id: u64, make: impl FnOnce() -> (E, L)) -> (&E, &mut L) {
        let i = match self
            .slots
            .iter()
            .position(|s| s.id == id && s.engine.is_some())
        {
            Some(i) => i,
            None => {
                let i = self.next;
                self.next = (self.next + 1) % CACHE_SLOTS;
                let (engine, locs) = make();
                self.slots[i] = Slot {
                    id,
                    engine: Some(engine),
                    locs: Some(locs),
                };
                i
            }
        };
        let slot = &mut self.slots[i];
        (
            slot.engine.as_ref().expect("set above"),
            slot.locs.as_mut().expect("set above"),
        )
    }
}

thread_local! {
    static TEXT_CACHE: RefCell<Cache<regex::Regex, regex::CaptureLocations>> =
        RefCell::new(Cache::new());
    static BYTES_CACHE: RefCell<Cache<regex::bytes::Regex, regex::bytes::CaptureLocations>> =
        RefCell::new(Cache::new());
}

/// Run `f` with this thread's clone of the text engine `re` and its
/// capture-location scratch. `id` names the engine: a pattern id, or
/// that id + 1 for the pattern's anchored `fullmatch` variant.
///
/// `f` runs while the thread's cache is borrowed and therefore must not
/// call back into anything that would take it again; in practice it only
/// drives the engine.
#[inline]
fn with_text_engine<R>(
    re: &regex::Regex,
    id: u64,
    f: impl FnOnce(&regex::Regex, &mut regex::CaptureLocations) -> R,
) -> R {
    TEXT_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        let (engine, locs) = cache.get(id, || (re.clone(), re.capture_locations()));
        f(engine, locs)
    })
}

/// Byte-mode counterpart of `with_text_engine`.
#[inline]
fn with_bytes_engine<R>(
    re: &regex::bytes::Regex,
    id: u64,
    f: impl FnOnce(&regex::bytes::Regex, &mut regex::bytes::CaptureLocations) -> R,
) -> R {
    BYTES_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        let (engine, locs) = cache.get(id, || (re.clone(), re.capture_locations()));
        f(engine, locs)
    })
}

/// Group spans of the leftmost-first match at or after `start`, copied
/// out of the thread's scratch for pattern `id`.
fn captures_spans_text(re: &regex::Regex, id: u64, hay: &str, start: usize) -> Option<Spans> {
    with_text_engine(re, id, |engine, locs| {
        engine.captures_read_at(locs, hay, start)?;
        Some(text_spans_of(locs))
    })
}

fn text_spans_of(locs: &regex::CaptureLocations) -> Spans {
    let mut spans = Spans::with_capacity(locs.len());
    for i in 0..locs.len() {
        spans.push(locs.get(i));
    }
    spans
}

/// Every match in the `[pb, eb)` byte window, as byte-unit group spans
/// in one flat buffer. With no capture groups the loop uses `find_at`
/// (no `Captures` allocation at all); with groups it uses
/// `captures_at` but still writes all matches into the same buffer.
/// Pure Rust, safe under a released GIL. Follows CPython's (>=3.7)
/// zero-width iteration rule via `cpython_step_flat`.
fn scan_all_flat_text(
    re: &regex::Regex,
    id: u64,
    retry: Option<&RetryEngine>,
    s: &str,
    pb: usize,
    eb: usize,
    groups: usize,
) -> Vec<Option<(usize, usize)>> {
    let hay = &s[..eb];
    let mut out = Vec::new();
    with_text_engine(re, id, |re, locs| {
        let mut scan = pb;
        let mut pending: Option<usize> = None;
        while scan <= eb {
            let start = out.len();
            let found = cpython_step_flat(
                hay.as_bytes(),
                scan,
                &mut pending,
                &mut out,
                |at, out| {
                    if groups == 0 {
                        match re.find_at(hay, at) {
                            Some(m) => {
                                out.push(Some((m.start(), m.end())));
                                true
                            }
                            None => false,
                        }
                    } else {
                        match re.captures_read_at(locs, hay, at) {
                            Some(_) => {
                                for i in 0..=groups {
                                    out.push(locs.get(i));
                                }
                                true
                            }
                            None => false,
                        }
                    }
                },
                retry,
                |p| next_boundary(s, p),
            );
            if !found {
                break;
            }
            pending = advance_after(&out[start..], &mut scan).or(pending);
        }
        out
    })
}

fn scan_all_flat_bytes(
    re: &regex::bytes::Regex,
    id: u64,
    retry: Option<&RetryEngine>,
    b: &[u8],
    pb: usize,
    eb: usize,
    groups: usize,
) -> Vec<Option<(usize, usize)>> {
    let hay = &b[..eb];
    let mut out = Vec::new();
    with_bytes_engine(re, id, |re, locs| {
        let mut scan = pb;
        let mut pending: Option<usize> = None;
        while scan <= eb {
            let start = out.len();
            let found = cpython_step_flat(
                hay,
                scan,
                &mut pending,
                &mut out,
                |at, out| {
                    if groups == 0 {
                        match re.find_at(hay, at) {
                            Some(m) => {
                                out.push(Some((m.start(), m.end())));
                                true
                            }
                            None => false,
                        }
                    } else {
                        match re.captures_read_at(locs, hay, at) {
                            Some(_) => {
                                for i in 0..=groups {
                                    out.push(locs.get(i));
                                }
                                true
                            }
                            None => false,
                        }
                    }
                },
                retry,
                |p| p + 1,
            );
            if !found {
                break;
            }
            pending = advance_after(&out[start..], &mut scan).or(pending);
        }
        out
    })
}

fn findall_list_text<'py>(
    py: Python<'py>,
    s: &str,
    groups: usize,
    spans: &[Option<(usize, usize)>],
    mapped: bool,
) -> PyResult<Bound<'py, PyList>> {
    if spans.is_empty() {
        return Ok(PyList::empty(py));
    }
    let stride = groups + 1;
    let mut items = Vec::with_capacity(spans.len() / stride);
    for chunk in spans.chunks_exact(stride) {
        items.push(findall_item_text(py, s, groups, chunk, mapped)?);
    }
    PyList::new(py, items)
}

fn findall_list_bytes<'py>(
    py: Python<'py>,
    b: &[u8],
    groups: usize,
    spans: &[Option<(usize, usize)>],
) -> PyResult<Bound<'py, PyList>> {
    if spans.is_empty() {
        return Ok(PyList::empty(py));
    }
    let stride = groups + 1;
    let mut items = Vec::with_capacity(spans.len() / stride);
    for chunk in spans.chunks_exact(stride) {
        items.push(findall_item_bytes(py, b, groups, chunk)?);
    }
    PyList::new(py, items)
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
    mapped: bool,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match groups {
        0 => {
            let (a, b) = spans[0].expect("group 0 always matches");
            text_result(py, &s[a..b], mapped)?.into_any()
        }
        1 => match spans[1] {
            Some((a, b)) => text_result(py, &s[a..b], mapped)?.into_any(),
            None => PyString::new(py, "").into_any(),
        },
        _ => {
            let parts: PyResult<Vec<Bound<'py, PyAny>>> = spans[1..=groups]
                .iter()
                .map(|sp| match sp {
                    Some((a, b)) => Ok(text_result(py, &s[*a..*b], mapped)?.into_any()),
                    None => Ok(PyString::new(py, "").into_any()),
                })
                .collect();
            PyTuple::new(py, parts?)?.into_any()
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

/// Byte-mode counterpart of `captures_spans_text`.
fn captures_spans_bytes(
    re: &regex::bytes::Regex,
    id: u64,
    hay: &[u8],
    start: usize,
) -> Option<Spans> {
    with_bytes_engine(re, id, |engine, locs| {
        engine.captures_read_at(locs, hay, start)?;
        let mut spans = Spans::with_capacity(locs.len());
        for i in 0..locs.len() {
            spans.push(locs.get(i));
        }
        Some(spans)
    })
}

fn offset_spans(spans: Spans, off: usize) -> Spans {
    spans.shifted(off)
}

/// Shared implementation of `sub` and `subn`.
pub(crate) fn substitute(
    this: &Py<Regex>,
    py: Python<'_>,
    repl: &Bound<'_, PyAny>,
    string: &Bound<'_, PyAny>,
    count: usize,
) -> PyResult<(Py<PyAny>, usize)> {
    let reg: &Regex = this.bind(py).get();
    let bytes_mode = matches!(reg.inner, Inner::Bytes(_));
    // Validate the replacement eagerly (CPython parses templates even
    // when nothing matches).
    enum Repl {
        Template(Vec<u8>, template::Template),
        Callable(Py<PyAny>),
    }
    let empty: &[Option<String>] = &[];
    let names_map: HashMap<&str, usize> =
        template::names_map(reg.names.as_deref().map_or(empty, |v| v.as_slice()));
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
            template::parse(bytes, reg.groups, &names_map)
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

    let mut keep = extract_hay(bytes_mode, string)?;
    let ascii = crate::utf8::is_ascii_obj(string);
    let hay = keep.hay(reg.stand_in)?;
    match (hay, &reg.inner) {
        (Hay::Text(s, mapped), Inner::Text(engine)) => {
            let names = reg.names.clone();
            let retry = reg.retry_engine().map(Arc::as_ref);
            let mut out = String::new();
            let mut prev = 0usize;
            let mut chars_prev = 0usize;
            let mut scan = 0usize;
            let mut n = 0usize;
            let mut pending: Option<usize> = None;
            loop {
                if count != 0 && n == count {
                    break;
                }
                let Some(spans) = per_step(|| {
                    cpython_step(
                        s.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, reg.id, s, at),
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
                        let mut py_spans = spans.clone();
                        to_py_spans(s, ascii, (prev, chars_prev), &mut py_spans);
                        let m = Match::new(
                            this.clone_ref(py),
                            string.clone().unbind(),
                            0,
                            Endpos::Eof(OnceLock::new()),
                            py_spans,
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
                chars_prev += s[prev..se].chars().count();
                prev = se;
                pending = advance_after(&spans, &mut scan).or(pending);
                n += 1;
            }
            out.push_str(&s[prev..]);
            Ok((text_result(py, &out, mapped)?.into_any().unbind(), n))
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
                let Some(spans) = per_step(|| {
                    cpython_step(
                        b,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, reg.id, b, at),
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
    /// Characters before `bytes_seen` in the text view: the per-step
    /// offset conversion counts from here instead of re-walking the
    /// haystack from its start (text mode only).
    chars_seen: usize,
    bytes_seen: usize,
    /// The transcoded UTF-8, kept for the iterator's lifetime when the
    /// cold path had to materialise it (see `utf8_of`). Borrowed (ASCII
    /// or warm) haystacks leave this empty and are re-borrowed per step.
    utf8: Option<crate::utf8::Utf8Scratch>,
    /// Whether the text view had a surrogate mapped to a stand-in (see
    /// `SreScanner::mapped`); carried into a scanner built from here.
    mapped: bool,
    names: Option<Arc<Vec<Option<String>>>>,
    /// The pattern's never-reused id, the key of its per-thread
    /// capture-location scratch.
    id: u64,
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
                let bound = self.string.bind(py);
                // One extraction per step: it is the cheap borrow path
                // (ASCII data or CPython's warm cache). Only when it
                // fails -- a haystack with a lone surrogate -- do we
                // transcode ourselves and keep the buffer for the
                // iterator's lifetime.
                let borrowed = if self.utf8.is_none() {
                    bound.extract::<&str>().ok()
                } else {
                    None
                };
                if self.utf8.is_none() && borrowed.is_none() {
                    if let crate::utf8::Utf8Buf::Owned(scratch) =
                        crate::utf8::utf8_of(bound.cast::<PyString>()?, re_ref.stand_in)?
                    {
                        self.mapped = scratch.mapped();
                        self.utf8 = Some(scratch);
                    }
                }
                let s: &str = match &self.utf8 {
                    Some(scratch) => scratch.as_str(),
                    None => borrowed.expect("extracted or transcoded above"),
                };
                let hay = &s[..self.end_b];
                let scan = self.scan;
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let ctx_id = self.id;
                let spans = per_step(|| {
                    cpython_step(
                        hay.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, ctx_id, hay, at),
                        retry,
                        |p| next_boundary(s, p),
                    )
                });
                let Some(spans) = spans else {
                    self.pending = pending;
                    self.done = true;
                    return Ok(None);
                };
                let (_, se) = spans[0].expect("group 0 always matches");
                let advance = advance_after(&spans, &mut self.scan);
                let ascii = crate::utf8::is_ascii_obj(self.string.bind(py));
                let mut spans = spans;
                to_py_spans(hay, ascii, (self.bytes_seen, self.chars_seen), &mut spans);
                self.chars_seen += s[self.bytes_seen..se].chars().count();
                self.bytes_seen = se;
                self.pending = advance.or(pending);
                Ok(Some(Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
                    spans,
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
                let ctx_id = self.id;
                let spans = per_step(|| {
                    cpython_step(
                        hay,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, ctx_id, hay, at),
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
    utf8: Option<crate::utf8::Utf8Scratch>,
    /// Whether the text view had a surrogate mapped to a stand-in; set
    /// on the first step from the transcoder.
    mapped: bool,
    names: Option<Arc<Vec<Option<String>>>>,
    /// The pattern's never-reused id, the key of its per-thread
    /// capture-location scratch.
    id: u64,
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
                let bound = self.string.bind(py);
                // One extraction per step: it is the cheap borrow path
                // (ASCII data or CPython's warm cache). Only when it
                // fails -- a haystack with a lone surrogate -- do we
                // transcode ourselves and keep the buffer for the
                // iterator's lifetime.
                let borrowed = if self.utf8.is_none() {
                    bound.extract::<&str>().ok()
                } else {
                    None
                };
                if self.utf8.is_none() && borrowed.is_none() {
                    if let crate::utf8::Utf8Buf::Owned(scratch) =
                        crate::utf8::utf8_of(bound.cast::<PyString>()?, re_ref.stand_in)?
                    {
                        self.mapped = scratch.mapped();
                        self.utf8 = Some(scratch);
                    }
                }
                let s: &str = match &self.utf8 {
                    Some(scratch) => scratch.as_str(),
                    None => borrowed.expect("extracted or transcoded above"),
                };
                let hay = &s[..self.end_b];
                let scan = self.scan;
                let retry = re_ref.retry_engine().map(Arc::as_ref);
                let mut pending = self.pending;
                let ctx_id = self.id;
                let spans = per_step(|| {
                    cpython_step(
                        hay.as_bytes(),
                        scan,
                        &mut pending,
                        |at| captures_spans_text(engine, ctx_id, hay, at),
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
                let ascii = crate::utf8::is_ascii_obj(self.string.bind(py));
                // Character offset of the cursor before this step (for
                // the anchored filter). Like CPython's SRE_Scanner, a
                // failed anchored call still consumes the match it
                // rejected — the state always advances.
                let cursor = if anchored {
                    Some(self.chars_seen + s[self.bytes_seen..scan].chars().count())
                } else {
                    None
                };
                let advance = advance_after(&spans, &mut self.scan);
                let mut spans = spans;
                to_py_spans(hay, ascii, (self.bytes_seen, self.chars_seen), &mut spans);
                self.chars_seen += s[self.bytes_seen..se].chars().count();
                self.bytes_seen = se;
                self.pending = advance.or(pending);
                let m = Match::new(
                    self.re.clone_ref(py),
                    self.string.clone_ref(py),
                    self.pos,
                    self.endpos.replicate(),
                    spans,
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
                let ctx_id = self.id;
                let spans = per_step(|| {
                    cpython_step(
                        hay,
                        scan,
                        &mut pending,
                        |at| captures_spans_bytes(engine, ctx_id, hay, at),
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
        let re_id = re_handle.bind(py).get().id;
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
            utf8: None,
            mapped: false,
            names: None,
            id: re_id,
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
            let token = text_result(py, &s[byte_start..state.bytes_seen], state.mapped)?;
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
        let rest = text_result(py, &s[i_byte..], state.mapped)?;
        Ok((PyList::new(py, result)?, rest.into_any()))
    }
}
