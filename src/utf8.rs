//! Three-way UTF-8 haystack borrow for str inputs.
//!
//! The engine matches UTF-8, but CPython stores `str` as UCS-1/2/4.
//! `PyUnicode_AsUTF8AndSize` (via PyO3's `to_str`) encodes-and-caches
//! a UTF-8 copy inside the string object on first use — GIL-held, at
//! CPython's scalar speed (~0.45 GB/s effective). For one-shot scans
//! of many short non-ASCII strings that encode dominates the call
//! (measured on 1M Telegram posts: ~1.0 s of a 1.58 s pile).
//!
//! This module avoids it when possible:
//!
//! 1. **Compact ASCII** — the UCS-1 data *is* valid UTF-8; borrow the
//!    internal buffer zero-copy (exactly what CPython itself does).
//! 2. **Non-ASCII, CPython's utf8 cache present** (some earlier
//!    C-API consumer — rurex's own previous scan, orjson, msgspec,
//!    sqlite, ...) — borrow the cached buffer, zero cost.
//! 3. **Non-ASCII, cache cold** — transcode UCS→UTF-8 in Rust
//!    into an owned buffer. On x86_64 with AVX2+BMI2 this uses the
//!    SIMD kernel in `utf8/simd.rs` (measured 2-4x the scalar
//!    encoder); everything else runs the scalar loop below. Either
//!    way CPython's cache is *not* populated: its buffer is
//!    PyMem_Malloc-owned and only `PyUnicode_AsUTF8AndSize` may
//!    write it under its own allocator rules. Re-scans of strings
//!    warmed only by rurex re-transcode at Rust speed — cheap.
//!
//! ## Object layout is detected, not assumed
//!
//! Paths 1 and 2 read PEP 393 fields through raw pointers, so the
//! field offsets must be right for the *running* interpreter. They
//! are not universal: the `PyObject` header is 16 bytes on a
//! GIL build but 32 on a free-threaded one (3.13t/3.14t), and
//! `PyCompactUnicodeObject` carried an extra `wstr_length` field
//! before 3.12. Hard-coding one layout silently mis-reads the others
//! — on a free-threaded build the old constants pointed at the
//! refcount, so the "compact ASCII" bit was read from a refcount
//! word and the peek either fell back (slow) or, on some layouts,
//! could have returned a bogus slice.
//!
//! So the layout is discovered once per process (`layout`):
//!
//! - the `length` field is located by matching the known length of
//!   two long ASCII probe strings against pointer-aligned offsets
//!   inside the object (long probes keep every read inside the
//!   allocation);
//! - everything else follows from it, since the PEP 393 field order
//!   is fixed: `length`, `hash`, `state`, then the compact fields
//!   `utf8_length` + `utf8`, then the UCS data (plus `wstr_length`
//!   before 3.12);
//! - the resulting layout is then **validated against CPython's own
//!   behaviour**: the `state` bits and ASCII data must match, a
//!   warm UTF-8 cache must equal `PyUnicode_AsUTF8AndSize`'s bytes,
//!   and a UCS-2 string's units must match its code points.
//!
//! If detection or validation fails, `layout` yields `None` and every
//! str haystack takes `to_cow` — CPython's encoder, exactly the
//! pre-optimization behaviour. The fast path is never entered on an
//! unverified layout, so a future CPython change degrades to "slow",
//! never to "wrong".
//!
//! state bits: `kind:3` << 2 (1/2/4 = UCS-1/2/4), `compact:1` << 5,
//! `ascii:1` << 6. The `utf8` field is written once (cache fill) and
//! never mutated; on free-threaded builds it is loaded atomically,
//! so we always load it atomically (a plain `Relaxed` load on GIL
//! builds compiles to the same instruction).

use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};

use crate::prefilter::Cold;
use std::slice;
use std::sync::OnceLock;

#[cfg(target_arch = "x86_64")]
mod simd;

// The `state` bitfield positions. On a GIL build `interned` is a
// 2-bit field, so `kind` starts at bit 2; on a free-threaded build
// CPython makes `interned` a whole `unsigned char` (it must be
// accessed atomically), which widens it to 8 bits and pushes `kind`,
// `compact` and `ascii` up by 6 (Include/cpython/unicodeobject.h,
// `Py_GIL_DISABLED`). The shift is therefore part of the detected
// layout, not a constant.
mod state_bits {
    pub const KIND_SHIFT_BASE: u32 = 2;
    pub const KIND_MASK: u32 = 0b111;
    pub const COMPACT_BASE: u32 = 5;
    pub const ASCII_BASE: u32 = 6;
}

/// The UCS storage kind (`PyUnicode_1BYTE_KIND` etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Ucs1,
    Ucs2,
    Ucs4,
}

/// PEP 393 field offsets for the running interpreter, in bytes from
/// the object pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// `Py_ssize_t length` of the ASCII/compact base object.
    pub length: usize,
    /// `unsigned int state` (bitfield).
    pub state: usize,
    /// `Py_ssize_t utf8_length` (compact strings only).
    pub utf8_length: usize,
    /// `char *utf8` — the lazily filled CPython UTF-8 cache.
    pub utf8: usize,
    /// UCS data start for compact ASCII strings.
    pub data_ascii: usize,
    /// UCS data start for compact non-ASCII strings.
    pub data_nonascii: usize,
    /// Extra bits before the `state` bitfields: 0 on a GIL build, 6
    /// on a free-threaded one (see `state_bits`).
    pub state_shift: u32,
}

impl Layout {
    /// `kind` field of a `state` word (1/2/4 = UCS-1/2/4).
    pub fn kind(&self, state: u32) -> u32 {
        (state >> (state_bits::KIND_SHIFT_BASE + self.state_shift)) & state_bits::KIND_MASK
    }

    /// Is the object compact (single allocation)?
    pub fn compact(&self, state: u32) -> bool {
        state & (1 << (state_bits::COMPACT_BASE + self.state_shift)) != 0
    }

    /// Is the string pure ASCII (and hence already valid UTF-8)?
    pub fn ascii(&self, state: u32) -> bool {
        state & (1 << (state_bits::ASCII_BASE + self.state_shift)) != 0
    }
}

impl Layout {
    /// Derive the remaining fields from the `length` offset. The field
    /// order is fixed by PEP 393: length, hash, state (4 bytes padded
    /// to 8), then the compact pair utf8_length/utf8, then the data.
    ///
    /// Before CPython 3.12 the object also carries an eight-byte
    /// `wstr_length` field, and the base object itself is eight bytes
    /// longer, so everything after `state` sits 8 bytes further out —
    /// including the ASCII data — and the non-ASCII data sits 16 bytes
    /// further out (8 for the longer base, 8 for `wstr_length`). Both
    /// shapes are measured on 3.10, 3.11, 3.12 and 3.13; `extra` is 0
    /// or 8 and `validate` decides which.
    const fn from_length(length: usize, extra: usize, state_shift: u32) -> Layout {
        Layout {
            length,
            state: length + 16,
            utf8_length: length + 24 + extra,
            utf8: length + 32 + extra,
            data_ascii: length + 24 + extra,
            data_nonascii: length + 40 + 2 * extra,
            state_shift,
        }
    }
}

fn read_u32(p: *const u8, off: usize) -> u32 {
    unsafe { (p.add(off) as *const u32).read_unaligned() }
}

fn read_usize(p: *const u8, off: usize) -> usize {
    unsafe { (p.add(off) as *const usize).read_unaligned() }
}

fn read_ptr(p: *const u8, off: usize) -> *const u8 {
    unsafe { (p.add(off) as *const *const u8).read_unaligned() }
}

/// Locate `length`, then validate the whole layout against CPython.
fn detect(py: Python<'_>) -> Option<Layout> {
    // Long ASCII probes: their allocations are larger than any
    // offset we scan, so every read stays inside the object.
    let short = PyString::new(py, &"x".repeat(300));
    let long = PyString::new(py, &"y".repeat(700));
    let a = short.as_ptr() as *const u8;
    let b = long.as_ptr() as *const u8;

    let mut found = None;
    // The header is 16 bytes on GIL builds and 32 on free-threaded
    // ones (3.13t/3.14t), so `length` sits at 16, 24 or 32.
    for off in [16usize, 24, 32, 40] {
        if read_usize(a, off) == 300 && read_usize(b, off) == 700 {
            found = Some(off);
            break;
        }
    }
    let length = found?;

    // `wstr_length` existed before 3.12 only; the `state` bitfields
    // shift by 8 on free-threaded builds. Decide both by validation
    // rather than by version sniffing.
    for state_shift in [0u32, 6] {
        for extra in [0usize, 8] {
            let l = Layout::from_length(length, extra, state_shift);
            if validate(py, &l) {
                return Some(l);
            }
        }
    }
    None
}

/// Prove the layout on this interpreter: state bits, ASCII data, a
/// warmed UTF-8 cache, and UCS-2 units must all match CPython.
fn validate(py: Python<'_>, l: &Layout) -> bool {
    // 1. Compact ASCII: state says compact|ascii, data is the text.
    let ascii = PyString::new(py, "abcde");
    let p = ascii.as_ptr() as *const u8;
    let state = read_u32(p, l.state);
    if !l.compact(state) || !l.ascii(state) {
        return false;
    }
    if read_usize(p, l.length) != 5 {
        return false;
    }
    if unsafe { slice::from_raw_parts(p.add(l.data_ascii), 5) } != b"abcde" {
        return false;
    }

    // 2. UCS-1 non-ASCII with the UTF-8 cache warmed by CPython:
    // the peeked bytes must be exactly the UTF-8 encoding.
    let text = "héllo wörld";
    let s = PyString::new(py, text);
    let p = s.as_ptr() as *const u8;
    let state = read_u32(p, l.state);
    if !l.compact(state) || l.ascii(state) || l.kind(state) != 1 {
        return false;
    }
    if read_usize(p, l.length) != text.chars().count() {
        return false;
    }
    if s.to_str().is_err() {
        return false;
    }
    let want = text.as_bytes();
    let cached_len = read_usize(p, l.utf8_length);
    let cached_ptr = read_ptr(p, l.utf8);
    if cached_len != want.len() || cached_ptr.is_null() {
        return false;
    }
    if unsafe { slice::from_raw_parts(cached_ptr, cached_len) } != want {
        return false;
    }

    // 3. UCS-2 data offset: the units must be the string's code
    // points, which is what distinguishes the pre-3.12 extra field.
    let arabic = "مرحبا";
    let s = PyString::new(py, arabic);
    let p = s.as_ptr() as *const u8;
    let state = read_u32(p, l.state);
    if l.kind(state) != 2 {
        return false;
    }
    let units: Vec<u32> = arabic.chars().map(|c| c as u32).collect();
    if read_usize(p, l.length) != units.len() {
        return false;
    }
    let got = unsafe { slice::from_raw_parts(p.add(l.data_nonascii) as *const u16, units.len()) };
    if got.iter().map(|&u| u as u32).ne(units.iter().copied()) {
        return false;
    }

    true
}

/// The validated layout for this process, or `None` (fast paths off).
pub(crate) fn layout(py: Python<'_>) -> Option<&'static Layout> {
    static LAYOUT: OnceLock<Option<Layout>> = OnceLock::new();
    LAYOUT.get_or_init(|| detect(py)).as_ref()
}

/// A reusable output buffer. The transcoder writes into it and the
/// caller reads it back as UTF-8 for the duration of one call.
///
/// Buffers come from a per-thread free list instead of the allocator:
/// a fresh `Vec` per call is not just a malloc, it is *cold cache* —
/// the kernel's stores then pay write-allocate misses on every line
/// (measured: most of the per-call overhead on short strings). Reusing
/// the same buffer keeps it resident in L1/L2 across calls.
pub(crate) struct Utf8Scratch {
    buf: Vec<u8>,
    len: usize,
    /// True when a lone surrogate was mapped to the stand-in block, so
    /// text results must be rebuilt with the surrogates restored.
    mapped: bool,
}

/// Retained buffers per thread, and the largest capacity kept (a
/// haystack-sized buffer is freed rather than pinned forever).
const POOL_MAX: usize = 4;
const POOL_CAP_MAX: usize = 1 << 20;

fn pool() -> &'static std::thread::LocalKey<std::cell::RefCell<Vec<Vec<u8>>>> {
    thread_local! {
        static POOL: std::cell::RefCell<Vec<Vec<u8>>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    &POOL
}

impl Utf8Scratch {
    /// Take a buffer with room for at least `cap` bytes.
    fn take(cap: usize) -> Utf8Scratch {
        let mut buf = pool().with(|p| p.borrow_mut().pop()).unwrap_or_default();
        buf.clear();
        buf.reserve(cap);
        Utf8Scratch {
            buf,
            len: 0,
            mapped: false,
        }
    }

    /// Whether the transcode replaced any lone surrogate.
    pub(crate) fn mapped(&self) -> bool {
        self.mapped
    }

    /// The UTF-8 written by the transcoder.
    pub(crate) fn as_str(&self) -> &str {
        // SAFETY: the transcoder writes valid UTF-8 (validated against
        // CPython's encoder by the differential tests).
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl Drop for Utf8Scratch {
    fn drop(&mut self) {
        let buf = std::mem::take(&mut self.buf);
        if buf.capacity() <= POOL_CAP_MAX {
            pool().with(|p| {
                let mut p = p.borrow_mut();
                if p.len() < POOL_MAX {
                    p.push(buf);
                }
            });
        }
    }
}

/// A borrowed-or-owned UTF-8 haystack for one call. The borrowed
/// form is valid while the caller keeps the `Bound<PyString>` alive
/// (the string object, and with it the internal/cache buffer, cannot
/// be freed).
pub(crate) enum Utf8Buf<'py> {
    Borrowed(&'py str, bool),
    /// Transcoded in Rust (cache was cold) or copied by CPython
    /// (layout undetected).
    Owned(Utf8Scratch),
    /// A cold transcode was avoided because the pattern's required
    /// first byte is absent from the UCS storage; no match is
    /// possible in this haystack.
    NoMatch,
}

impl Utf8Buf<'_> {
    /// The UTF-8 view, whichever variant this is (tests only; the
    /// call sites read the scratch directly).
    #[cfg(test)]
    fn as_str(&self) -> &str {
        match self {
            Utf8Buf::Borrowed(s, _) => s,
            Utf8Buf::Owned(s) => s.as_str(),
            Utf8Buf::NoMatch => unreachable!("tests only pass haystacks"),
        }
    }
}

/// Which UCS kind / cache state the string is in, per the layout.
enum Stored {
    /// Borrowable UTF-8 right now.
    Utf8(&'static str),
    /// UCS data ready to transcode.
    Transcode(Kind, usize, *const u8),
    /// Layout mismatch for this object (non-compact legacy string).
    Unknown,
}

/// Inspect the string object: borrow UTF-8 when it is already there,
/// otherwise report the UCS buffer to transcode.
fn inspect<'py>(bound: &Bound<'py, PyString>, l: &Layout) -> Stored {
    let p = bound.as_ptr() as *const u8;
    let state = read_u32(p, l.state);
    if !l.compact(state) {
        return Stored::Unknown;
    }
    let len = read_usize(p, l.length);
    if l.ascii(state) {
        // Compact ASCII: one byte per char, valid UTF-8 by
        // construction.
        let bytes = unsafe { slice::from_raw_parts(p.add(l.data_ascii), len) };
        return Stored::Utf8(unsafe { std::str::from_utf8_unchecked(bytes) });
    }
    // Compact non-ASCII: is CPython's UTF-8 cache warm?
    let cached = unsafe {
        (p.add(l.utf8) as *const std::sync::atomic::AtomicPtr<u8>)
            .as_ref()
            .expect("non-null address")
            .load(std::sync::atomic::Ordering::Acquire)
    };
    if !cached.is_null() {
        let cached_len = read_usize(p, l.utf8_length);
        let bytes = unsafe { slice::from_raw_parts(cached, cached_len) };
        // CPython guarantees the cache is valid UTF-8.
        return Stored::Utf8(unsafe { std::str::from_utf8_unchecked(bytes) });
    }
    let kind = match l.kind(state) {
        1 => Kind::Ucs1,
        2 => Kind::Ucs2,
        4 => Kind::Ucs4,
        // Not a kind CPython produces: take the slow, safe route.
        _ => return Stored::Unknown,
    };
    Stored::Transcode(kind, len, unsafe { p.add(l.data_nonascii) })
}

/// True when `obj` is a `str` whose compact PEP 393 storage is ASCII.
///
/// That means its UTF-8 view has exactly one byte per character, so
/// engine byte offsets already are Python character offsets and the
/// result path can skip offset conversion entirely. O(1): two field
/// reads and no scanning. Anything else (a non-ASCII `str`, a
/// non-`str`, or an object whose layout was not validated) answers
/// `false`; callers then convert offsets the slow way, which is
/// always correct.
pub(crate) fn is_ascii_obj(obj: &Bound<'_, PyAny>) -> bool {
    let Ok(s) = obj.cast::<PyString>() else {
        return false;
    };
    let Some(l) = layout(obj.py()) else {
        return false;
    };
    let state = read_u32(s.as_ptr() as *const u8, l.state);
    l.compact(state) && l.ascii(state)
}

/// Three-way borrow: ASCII/cache zero-copy, cold non-ASCII transcoded
/// in Rust. When the object layout could not be validated (or the
/// string is a non-compact legacy object) this falls back to
/// CPython's encoder via `to_cow`.
pub(crate) fn utf8_of<'py>(bound: &Bound<'py, PyString>, stand_in: bool) -> PyResult<Utf8Buf<'py>> {
    utf8_of_filtered(bound, None, stand_in)
}

/// Like `utf8_of`, but on the cold-transcode path consult the
/// pattern's first-byte prefilter first: when the pattern's required
/// first byte is absent from the UCS storage, return `NoMatch` instead
/// of transcoding. `cold` is the lazily derived prefilter slot, so a
/// pattern is only parsed for it once a cold string actually needs the
/// test (ADR-0015). The prefilter is never applied to ASCII strings,
/// warm CPython caches, or objects whose layout could not be validated.
pub(crate) fn utf8_of_filtered<'py>(
    bound: &Bound<'py, PyString>,
    cold: Option<&Cold<'_>>,
    stand_in: bool,
) -> PyResult<Utf8Buf<'py>> {
    if let Some(l) = layout(bound.py()) {
        match inspect(bound, l) {
            Stored::Utf8(s) => return Ok(Utf8Buf::Borrowed(s, false)),
            Stored::Transcode(kind, len, data) => {
                if let Some(p) = cold.and_then(Cold::first) {
                    // SAFETY: `data`/`len`/`kind` come from `inspect`
                    // under the validated layout.
                    if !unsafe { p.may_match_ucs(kind, len, data) } {
                        return Ok(Utf8Buf::NoMatch);
                    }
                }
                return Ok(Utf8Buf::Owned(transcode_maybe_detached(
                    bound.py(),
                    kind,
                    len,
                    data,
                    stand_in,
                )?));
            }
            Stored::Unknown => {}
        }
    }
    let cow = bound.to_cow()?;
    let mut scratch = Utf8Scratch::take(cow.len());
    scratch.buf.extend_from_slice(cow.as_bytes());
    scratch.len = cow.len();
    Ok(Utf8Buf::Owned(scratch))
}

/// Base of the private-use block standing in for lone surrogates.
///
/// `re` treats a lone surrogate as an ordinary code point. Private-use
/// characters are indistinguishable from surrogates across `re`'s whole
/// class vocabulary -- not `\w`, `\d` or `\s`, matched by `.` and by
/// negated classes, no case mapping -- and the block is 2048 wide,
/// exactly the surrogate range, so the mapping is one code point per
/// code point and every offset stays valid. Plane 15 PUA-A is used
/// because ordinary text does not contain it; a string or pattern that
/// does is refused (see `utf8_of_filtered`'s callers), which is the one
/// remaining corner where a surrogate still raises.
pub(crate) const SURROGATE_BASE: u32 = 0xF0000;

/// Map a lone surrogate (U+D800..U+DFFF) into the stand-in block.
#[inline]
pub(crate) fn surrogate_stand_in(u: u32) -> u32 {
    SURROGATE_BASE + (u - 0xD800)
}

/// Is `cp` inside the stand-in block?
#[inline]
pub(crate) fn in_stand_in_block(cp: u32) -> bool {
    (SURROGATE_BASE..SURROGATE_BASE + 0x800).contains(&cp)
}

/// Does the pattern permit the surrogate stand-in mapping?
///
/// False when the pattern can tell a stand-in character from a
/// surrogate or from a real character: it names a code point in the
/// block (literally or through a `\x{...}` escape), or it asks for a
/// property that separates private use (Co) from surrogates (Cs). Such
/// a pattern keeps the old behaviour -- a lone surrogate in the
/// haystack raises -- which is the only residual corner.
pub(crate) fn pattern_allows_stand_in(pattern: &str) -> bool {
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if in_stand_in_block(c as u32) {
            return false;
        }
        if c != '\\' {
            continue;
        }
        match chars.next() {
            Some('x') if chars.peek() == Some(&'{') => {
                chars.next();
                let mut value = 0u32;
                let mut digits = 0;
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    let v = match d.to_digit(16) {
                        Some(v) => v,
                        None => return true,
                    };
                    value = value.saturating_mul(16).saturating_add(v);
                    digits += 1;
                }
                if digits > 0 && in_stand_in_block(value) {
                    return false;
                }
            }
            Some('p') | Some('P') if chars.peek() == Some(&'{') => {
                chars.next();
                let mut name = String::new();
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    name.push(d);
                }
                let tail = name.rsplit('=').next().unwrap_or("").trim();
                if tail == "Co" || tail == "Cs" {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn surrogate_error() -> PyErr {
    PyUnicodeEncodeError::new_err((
        "utf-8",
        "'surrogates not allowed'",
        0,
        0,
        "surrogates not allowed",
    ))
}

/// Source size at which the GIL is released around the transcode.
/// Short strings (the Telegram corpus) stay GIL-held: the handoff costs
/// more than the encode. Long documents (the homepage corpus) cross
/// this and let other Python threads run during the encode.
const TRANSCODE_DETACH_MIN_BYTES: usize = 32 * 1024;

/// Transcode, releasing the GIL first when the source is large enough
/// to pay for the handoff.
///
/// The caller passes the pointer from `inspect` under the validated
/// layout; `utf8_of_filtered` holds the `Bound` string across the
/// detach, and `str` is immutable, so the UCS buffer cannot move or be
/// freed while another Python thread runs.
fn transcode_maybe_detached(
    py: Python<'_>,
    kind: Kind,
    len: usize,
    data: *const u8,
    stand_in: bool,
) -> PyResult<Utf8Scratch> {
    let source_bytes = len
        * match kind {
            Kind::Ucs1 => 1,
            Kind::Ucs2 => 2,
            Kind::Ucs4 => 4,
        };
    if source_bytes < TRANSCODE_DETACH_MIN_BYTES {
        return unsafe { transcode(kind, len, data, stand_in) };
    }
    let addr = data as usize;
    py.detach(move || unsafe { transcode(kind, len, addr as *const u8, stand_in) })
}

/// Transcode UCS-1/2/4 to UTF-8 in Rust (cache was cold), into a
/// reused scratch buffer.
///
/// On x86_64 CPUs with AVX2+BMI2 the SIMD kernel in `simd` handles
/// the bulk; the scalar loop is the portable fallback, the error
/// path, and the reference the kernel is differentially tested
/// against.
unsafe fn transcode(
    kind: Kind,
    len: usize,
    data: *const u8,
    stand_in: bool,
) -> PyResult<Utf8Scratch> {
    let cap = (match kind {
        Kind::Ucs1 => len * 2,
        Kind::Ucs2 => len * 3,
        Kind::Ucs4 => len * 4,
    }) + 32;
    let mut scratch = Utf8Scratch::take(cap);
    let mut mapped = false;
    let n = unsafe {
        #[cfg(target_arch = "x86_64")]
        if simd::supported() {
            simd::transcode(kind, len, data, stand_in, &mut mapped, &mut scratch.buf)?
        } else {
            transcode_scalar(kind, len, data, stand_in, &mut mapped, &mut scratch.buf)?
        }
        #[cfg(not(target_arch = "x86_64"))]
        transcode_scalar(kind, len, data, stand_in, &mut mapped, &mut scratch.buf)?
    };
    scratch.len = n;
    scratch.mapped = mapped;
    Ok(scratch)
}

/// Scalar UCS→UTF-8 encoder: one unit at a time, appended to `out`.
/// Returns the number of bytes written. Surrogates (UCS-2/UCS-4) and
/// values above U+10FFFF (UCS-4) raise `UnicodeEncodeError`, like
/// `PyUnicode_AsUTF8AndSize`.
unsafe fn transcode_scalar(
    kind: Kind,
    len: usize,
    data: *const u8,
    stand_in: bool,
    mapped: &mut bool,
    out: &mut Vec<u8>,
) -> PyResult<usize> {
    let start = out.len();
    let s = unsafe {
        match kind {
            Kind::Ucs1 => {
                // Latin-1: each byte is a code point < 256.
                let data = slice::from_raw_parts(data, len);
                data.iter().map(|&b| b as char).collect::<String>()
            }
            Kind::Ucs2 => {
                // NOT `String::from_utf16`: it would merge adjacent
                // lone surrogates into an astral code point, but in
                // UCS-2 storage (CPython picks the kind by max code
                // point) every surrogate unit is a lone surrogate
                // *code point* and must raise, exactly like
                // `PyUnicode_AsUTF8AndSize`.
                // UCS-2 storage cannot contain a stand-in block code
                // point (the block is astral), so mapping a lone
                // surrogate can never alias a real character here.
                let units = slice::from_raw_parts(data as *const u16, len);
                let mut out = String::with_capacity(len * 3);
                for &u in units {
                    let cp = u as u32;
                    let c = match char::from_u32(cp) {
                        Some(c) => c,
                        None if stand_in && cp <= 0xDFFF => {
                            *mapped = true;
                            char::from_u32(surrogate_stand_in(cp)).expect("stand-in is valid")
                        }
                        None => return Err(surrogate_error()),
                    };
                    out.push(c);
                }
                out
            }
            Kind::Ucs4 => {
                // UCS-4 storage can hold a real character from the
                // stand-in block. Mapping a surrogate next to one would
                // make them indistinguishable, so the first such
                // character disables substitution for the whole string
                // (and a later surrogate raises, as before).
                let units = slice::from_raw_parts(data as *const u32, len);
                let mut out = String::with_capacity(len);
                let mut substituting = stand_in;
                for &u in units {
                    if substituting && in_stand_in_block(u) {
                        substituting = false;
                    }
                    let c = match char::from_u32(u) {
                        Some(c) => c,
                        None if substituting && (0xD800..0xE000).contains(&u) => {
                            *mapped = true;
                            char::from_u32(surrogate_stand_in(u)).expect("stand-in is valid")
                        }
                        None => return Err(surrogate_error()),
                    };
                    out.push(c);
                }
                out
            }
        }
    };
    out.extend_from_slice(s.as_bytes());
    Ok(out.len() - start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8_of_str(py: Python<'_>, s: &str) -> String {
        let obj = PyString::new(py, s);
        utf8_of(&obj, false).unwrap().as_str().to_string()
    }

    fn py_of(s: &str) -> String {
        Python::attach(|py| utf8_of_str(py, s))
    }

    /// Build a Python str from raw code points — the only way to
    /// create lone surrogates, since a Rust `str` cannot hold them.
    fn py_str_from_cps<'py>(py: Python<'py>, cps: &[u32]) -> Bound<'py, PyString> {
        let list = cps
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let code =
            std::ffi::CString::new(format!("\"\".join(map(chr, [{list}]))")).expect("no NUL");
        let obj = py
            .eval(&code, None, None)
            .expect("chr() of valid code points");
        obj.cast_into::<PyString>().expect("join of chr is str")
    }

    /// Layout detection must succeed on every interpreter we ship
    /// for — this is the guard that keeps the field peeking honest
    /// (a GIL build, a free-threaded build, and pre-3.12 all differ).
    #[test]
    fn layout_detected_and_validated() {
        Python::attach(|py| {
            let l = layout(py).expect("PEP 393 layout not detected/validated");
            // The derived offsets must be self-consistent: hash and
            // state are fixed, and the pre-3.12 shapes add 8 or 16
            // bytes after `state`.
            assert_eq!(l.state, l.length + 16);
            let extra = l.utf8_length - (l.length + 24);
            assert!(matches!(extra, 0 | 8), "extra = {extra}");
            assert_eq!(l.utf8, l.length + 32 + extra);
            assert_eq!(l.data_ascii, l.length + 24 + extra);
            assert_eq!(l.data_nonascii, l.length + 40 + 2 * extra);
            assert!(l.data_nonascii > l.utf8);
            assert!(matches!(l.state_shift, 0 | 6));
        });
    }

    /// Byte-equality with CPython's own encoder over a torture
    /// corpus: Arabic (UCS-2), emoji/astral (UCS-4), European
    /// latin-1, pure ASCII, embedded NUL, mixed scripts, long runs.
    #[test]
    fn transcode_matches_cpython_encoder() {
        let cases: Vec<String> = vec![
            "hello world".to_string(),
            "café naïve résumé".to_string(),         // latin-1 → Ucs1
            "مرحبا بالعالم خط عريض جدا".to_string(), // Arabic BMP
            "emoji: 🎉🚀😀👍🏽 family 👨‍👩‍👧‍👦".to_string(), // astral + ZWJ
            "mixed مرحبا café 🎉 ascii".to_string(),
            "with\0embedded\0nul".to_string(),
            "русский текст".to_string(),
            "日本語のテキスト、漢字とかな".to_string(),
            String::new(),
            "x".repeat(10_000),
            "مرحبا".repeat(5_000),
        ];
        for s in cases {
            let via_rust = py_of(&s);
            let via_py = Python::attach(|py| {
                let obj = PyString::new(py, &s);
                // The authoritative check: Python's own utf-8 encode.
                obj.to_str().unwrap().as_bytes().to_vec()
            });
            assert_eq!(
                via_rust.as_bytes(),
                via_py.as_slice(),
                "case {:?}",
                s.chars().take(20).collect::<String>()
            );
            assert_eq!(via_rust, s);
        }
    }

    /// Randomized differential fuzz vs CPython's own encoder over
    /// raw code points (surrogates included): both must agree on
    /// the verdict, and on success byte for byte (cold path, so the
    /// transcoder runs).
    #[test]
    fn fuzz_transcode_matches_cpython_encoder() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self) -> u64 {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                self.0 >> 11
            }
        }
        let alphabet: &[u32] = &[
            0x61, 0x5A, 0x30, 0x20, 0x0A, 0x00, // ASCII + space + NUL
            0xE9, 0xEF, 0x100, // Latin-1 supplement
            0x645, 0x644, 0x639, // Arabic
            0x65E5, 0x672C, 0x8A9E, // CJK
            0x1F389, 0x1F680, 0x1F3FD, // astral emoji
            0xD800, 0xDFFF, 0xD82A, 0xDEE0, // lone surrogates
        ];
        let mut rng = Rng(0x5EED_1234);
        for _ in 0..4000 {
            let len = (rng.next() % 80) as usize;
            let cps: Vec<u32> = (0..len)
                .map(|_| alphabet[(rng.next() as usize) % alphabet.len()])
                .collect();
            Python::attach(|py| {
                let obj = py_str_from_cps(py, &cps);
                let via_rust = utf8_of(&obj, false);
                let via_cpython = obj.to_str();
                match (via_rust, via_cpython) {
                    (Ok(a), Ok(b)) => assert_eq!(a.as_str(), b, "cps {cps:?}"),
                    (Err(_), Err(_)) => {}
                    (a, b) => panic!(
                        "verdict mismatch cps {cps:?}: ok={} cpython_ok={}",
                        a.is_ok(),
                        b.is_ok()
                    ),
                }
            });
        }
    }

    /// Every kernel-chunk boundary shape (the 8-unit SIMD loop and
    /// the scalar tail) over mixed-width content.
    #[test]
    fn chunk_boundaries_match_cpython_encoder() {
        let alphabet: Vec<char> = "aéل文🎉\0".chars().collect();
        for len in 0usize..=40 {
            for pattern in 0..7usize {
                let s: String = (0..len)
                    .map(|i| alphabet[(i + pattern) % alphabet.len()])
                    .collect();
                let via_rust = py_of(&s);
                let via_cpython = Python::attach(|py| {
                    let obj = PyString::new(py, &s);
                    obj.to_str().unwrap().as_bytes().to_vec()
                });
                assert_eq!(via_rust.as_bytes(), via_cpython.as_slice(), "len {len}");
            }
        }
    }

    /// A lone surrogate at every position of a mixed string must
    /// raise UnicodeEncodeError (cold, via utf8_of), matching
    /// `PyUnicode_AsUTF8AndSize`. Includes the adjacent-lone-
    /// surrogate pair (0xD82A, 0xDEE0) — two code points, NOT a
    /// UTF-16 pair (regression: the scalar path used to merge them
    /// via `String::from_utf16`).
    #[test]
    fn lone_surrogates_raise_at_every_position() {
        Python::attach(|py| {
            let base: Vec<u32> = "مرحبا abc 🎉 نص ".chars().map(|c| c as u32).collect();
            for pos in [0usize, 1, 3, 7, 8, 9, 15, 16, 17, 31] {
                for bad in [0xD800u32, 0xDFFF, 0xD82A] {
                    let mut cps = base.clone();
                    cps.insert(pos.min(cps.len()), bad);
                    let obj = py_str_from_cps(py, &cps);
                    let e = match utf8_of(&obj, false) {
                        Err(e) => e,
                        Ok(_) => panic!("surrogates must raise"),
                    };
                    let t = e.value(py).get_type().to_string();
                    assert!(t.contains("UnicodeEncodeError"), "got {t}");
                    // Warm-cache variant raises too (CPython's own
                    // encoder rejects it as well).
                    assert!(obj.to_str().is_err());
                }
            }
            // Adjacent lone surrogates (a UTF-16 pair by bytes).
            let obj = py_str_from_cps(py, &[0xD82A, 0xDEE0]);
            assert!(utf8_of(&obj, false).is_err());
            assert!(obj.to_str().is_err());
        });
    }

    /// The cache peek must hit after any C-API consumer touched the
    /// string — validates the detected offsets at the running
    /// interpreter.
    #[test]
    fn peek_hits_cache_after_to_str() {
        Python::attach(|py| {
            let s = "مرحبا بالعالم"; // non-ASCII, cache starts cold
            let obj = PyString::new(py, s);
            let l = layout(py).expect("layout");
            assert!(matches!(inspect(&obj, l), Stored::Transcode(..)));
            let _ = obj.to_str(); // CPython populates the cache
            match inspect(&obj, l) {
                Stored::Utf8(got) => assert_eq!(got, s),
                _ => panic!("cache must be warm now"),
            }
            match utf8_of(&obj, false).unwrap() {
                Utf8Buf::Borrowed(got, _) => assert_eq!(got, s),
                Utf8Buf::Owned(_) => panic!("warm non-ASCII must borrow"),
                Utf8Buf::NoMatch => panic!("no prefilter was passed"),
            }
        });
    }

    /// ASCII strings must borrow the internal buffer (never
    /// transcode, never touch the cache field).
    #[test]
    fn ascii_borrows_internal_buffer() {
        Python::attach(|py| {
            let obj = PyString::new(py, "hello world 123");
            match utf8_of(&obj, false).unwrap() {
                Utf8Buf::Borrowed(s, _) => assert_eq!(s, "hello world 123"),
                Utf8Buf::Owned(_) => panic!("ASCII must borrow, not own"),
                Utf8Buf::NoMatch => panic!("no prefilter was passed"),
            }
        });
    }

    /// Cold non-ASCII must take the owned (Rust transcode) path.
    #[test]
    fn cold_non_ascii_transcodes() {
        Python::attach(|py| {
            let obj = PyString::new(py, "مرحبا");
            match utf8_of(&obj, false).unwrap() {
                Utf8Buf::Owned(s) => assert_eq!(s.as_str(), "مرحبا"),
                Utf8Buf::Borrowed(..) => panic!("cold non-ASCII must own"),
                Utf8Buf::NoMatch => panic!("no prefilter was passed"),
            }
        });
    }
}
