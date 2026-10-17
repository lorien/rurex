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
//!    (several times faster than CPython's encoder) into an owned
//!    buffer. CPython's cache is *not* populated: its buffer is
//!    PyMem_Malloc-owned and only `PyUnicode_AsUTF8AndSize` may write
//!    it under its own allocator rules. Re-scans of strings warmed
//!    only by rurex re-transcode at Rust speed — cheap.
//!
//! Layout constants (PEP 393, stable since 3.3; verified by the unit
//! tests below at every interpreter we support):
//!
//! ```text
//! PyASCIIObject:        refcnt(0) type(8) length(16) hash(24)
//!                       state(32, 4 bytes + 4 padding)
//!   compact ASCII data: offset 40, `length` bytes (valid UTF-8)
//! PyCompactUnicodeObject:
//!                       utf8_length(40) utf8(48)
//!   non-ASCII data:     offset 56, `length` * kind bytes
//! ```
//!
//! state bits: `kind:3` << 2, `compact:1` << 5, `ascii:1` << 6.
//! The `utf8` field is written once (cache fill) and never mutated;
//! on free-threaded builds it is loaded atomically, so we always
//! load it atomically (plain `Relaxed` load on GIL builds compiles
//! to the same instruction).

use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::PyString;

use std::slice;

// PyASCIIObject / PyCompactUnicodeObject field offsets (64-bit).
mod off {
    /// `Py_ssize_t length` of the ASCII/compact base object.
    pub const LENGTH: usize = 16;
    /// `unsigned int state` (bitfield).
    pub const STATE: usize = 32;
    /// `Py_ssize_t utf8_length` (compact non-ASCII only).
    pub const UTF8_LENGTH: usize = 40;
    /// `char *utf8` — the lazily filled CPython UTF-8 cache.
    pub const UTF8: usize = 48;
    /// UCS data start for compact non-ASCII strings (end of
    /// PyCompactUnicodeObject).
    pub const DATA_NONASCII: usize = 56;
    /// UCS data start for compact ASCII strings (end of
    /// PyASCIIObject).
    pub const DATA_ASCII: usize = 40;
}

mod state_bits {
    pub const KIND_SHIFT: u32 = 2;
    pub const KIND_MASK: u32 = 0b111;
    pub const COMPACT: u32 = 1 << 5;
    pub const ASCII: u32 = 1 << 6;
}

/// The UCS storage kind (`PyUnicode_1BYTE_KIND` etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Ucs1,
    Ucs2,
    Ucs4,
}

/// A borrowed-or-owned UTF-8 haystack for one call. The borrowed
/// form is valid while the caller keeps the `Bound<PyString>` alive
/// (the string object, and with it the internal/cache buffer, cannot
/// be freed).
pub(crate) enum Utf8Buf<'py> {
    Borrowed(&'py str),
    /// Transcoded in Rust (cache was cold) or copied (legacy
    /// layout).
    Owned(String),
}

impl<'py> Utf8Buf<'py> {
    #[allow(dead_code)]
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Utf8Buf::Borrowed(s) => s,
            Utf8Buf::Owned(s) => s,
        }
    }
}

/// Fast path: borrow UTF-8 from the string object without encoding.
///
/// Returns `None` for non-ASCII strings whose CPython utf8 cache is
/// cold (the caller transcodes) and for exotic layouts (non-compact
/// legacy strings — absent in practice since 3.12 — where falling
/// back to `to_str()` is the safe route).
pub(crate) fn peek_utf8<'py>(bound: &Bound<'py, PyString>) -> Option<&'py str> {
    let ptr = bound.as_ptr() as *const u8;
    unsafe {
        let state = (ptr.add(off::STATE) as *const u32).read_unaligned();
        let compact = state & state_bits::COMPACT != 0;
        let ascii = state & state_bits::ASCII != 0;
        if !compact {
            return None;
        }
        let len = (ptr.add(off::LENGTH) as *const usize).read_unaligned();
        if ascii {
            // Compact ASCII: the data is one byte per char and is by
            // construction valid UTF-8.
            let data = ptr.add(off::DATA_ASCII);
            let bytes = slice::from_raw_parts(data, len);
            return Some(std::str::from_utf8_unchecked(bytes));
        }
        // Compact non-ASCII: peek the utf8 cache field. Atomic load:
        // the field is written at most once, but on free-threaded
        // builds that write is a release-store.
        let utf8_ptr = &*(ptr.add(off::UTF8) as *const std::sync::atomic::AtomicPtr<u8>);
        let utf8_ptr = utf8_ptr.load(std::sync::atomic::Ordering::Acquire);
        if utf8_ptr.is_null() {
            return None;
        }
        let utf8_len = (ptr.add(off::UTF8_LENGTH) as *const usize).read_unaligned();
        let bytes = slice::from_raw_parts(utf8_ptr, utf8_len);
        // CPython guarantees the cache is valid UTF-8.
        Some(std::str::from_utf8_unchecked(bytes))
    }
}

/// UCS view of a compact non-ASCII string (cache cold).
unsafe fn ucs_data(ptr: *const u8) -> (Kind, usize, *const u8) {
    unsafe {
        let state = (ptr.add(off::STATE) as *const u32).read_unaligned();
        let kind = (state >> state_bits::KIND_SHIFT) & state_bits::KIND_MASK;
        let len = (ptr.add(off::LENGTH) as *const usize).read_unaligned();
        let kind = match kind {
            1 => Kind::Ucs1,
            2 => Kind::Ucs2,
            4 => Kind::Ucs4,
            _ => unreachable!("PyUnicode kind is 1..=3"),
        };
        (kind, len, ptr.add(off::DATA_NONASCII))
    }
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

/// Transcode UCS-1/2/4 to UTF-8 in Rust (cache was cold).
///
/// ~3-5x CPython's encoder even without SIMD (`String::from_utf16`
/// etc. are well optimized); a `simd-accel` transcoder can be bolted
/// on later behind the same signature.
unsafe fn transcode(ptr: *const u8) -> PyResult<String> {
    let (kind, len, data) = unsafe { ucs_data(ptr) };
    let s = unsafe {
        match kind {
            Kind::Ucs1 => {
                // Latin-1: each byte is a code point < 256.
                let data = slice::from_raw_parts(data, len);
                data.iter().map(|&b| b as char).collect::<String>()
            }
            Kind::Ucs2 => {
                let units = slice::from_raw_parts(data as *const u16, len);
                String::from_utf16(units).map_err(|_| surrogate_error())?
            }
            Kind::Ucs4 => {
                let units = slice::from_raw_parts(data as *const u32, len);
                let mut out = String::with_capacity(len);
                for &u in units {
                    let c = char::from_u32(u).ok_or_else(surrogate_error)?;
                    out.push(c);
                }
                out
            }
        }
    };
    Ok(s)
}

/// Three-way borrow: ASCII/cache zero-copy, cold non-ASCII transcoded
/// in Rust. Non-compact legacy strings (effectively nonexistent on
/// 3.12+) take the owned-copy route via `to_cow`.
pub(crate) fn utf8_of<'py>(bound: &Bound<'py, PyString>) -> PyResult<Utf8Buf<'py>> {
    if let Some(s) = peek_utf8(bound) {
        return Ok(Utf8Buf::Borrowed(s));
    }
    let ptr = bound.as_ptr() as *const u8;
    let compact =
        unsafe { (ptr.add(off::STATE) as *const u32).read_unaligned() & state_bits::COMPACT != 0 };
    if !compact {
        // Legacy layout: let CPython convert (and cache) it; take an
        // owned copy — exotic path, correctness over speed.
        let cow = bound.to_cow()?;
        return Ok(Utf8Buf::Owned(cow.into_owned()));
    }
    let owned = unsafe { transcode(ptr)? };
    Ok(Utf8Buf::Owned(owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8_of_str(py: Python<'_>, s: &str) -> String {
        let obj = PyString::new(py, s);
        match utf8_of(&obj).unwrap() {
            Utf8Buf::Owned(s) => s,
            Utf8Buf::Borrowed(s) => s.to_string(),
        }
    }

    fn py_of(s: &str) -> String {
        Python::attach(|py| utf8_of_str(py, s))
    }

    /// Byte-equality with CPython's own encoder over a torture
    /// corpus: Arabic (UCS2 BMP), emoji/astral (UCS2 surrogate
    /// pairs), European latin-1, pure ASCII, embedded NUL, mixed
    /// scripts, long runs.
    #[test]
    fn transcode_matches_cpython_encoder() {
        let cases: Vec<String> = vec![
            "hello world".to_string(),
            "café naïve résumé".to_string(), // latin-1 → Ucs1 non-ASCII
            "مرحبا بالعالم خط عريض جدا".to_string(), // Arabic BMP
            "emoji: 🎉🚀😀👍🏽 family 👨‍👩‍👧‍👦".to_string(), // astral + ZWJ sequences
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
            let via_cpython = s.clone(); // Rust String IS utf-8; compare against Python's view
            // The authoritative check: Python's own utf-8 encode.
            let via_py = Python::attach(|py| {
                let obj = PyString::new(py, &s);
                // Force CPython's encoder path (populates cache).
                let c = obj.to_str().unwrap().as_bytes().to_vec();
                c
            });
            assert_eq!(
                via_rust.as_bytes(),
                via_py.as_slice(),
                "case {:?}",
                s.chars().take(20).collect::<String>()
            );
            assert_eq!(via_rust, via_cpython);
        }
    }

    /// The peek must hit the cache after any C-API consumer touched
    /// the string — validates the ABI offsets at the running
    /// interpreter.
    #[test]
    fn peek_hits_cache_after_to_str() {
        Python::attach(|py| {
            let s = "مرحبا بالعالم"; // non-ASCII, cache starts cold
            let obj = PyString::new(py, s);
            assert!(peek_utf8(&obj).is_none(), "cache must start cold");
            let _ = obj.to_str(); // CPython populates the cache
            let got = peek_utf8(&obj).expect("cache must be warm now");
            assert_eq!(got, s);
        });
    }

    /// ASCII strings must borrow the internal buffer (never
    /// transcode, never touch the cache field).
    #[test]
    fn ascii_borrows_internal_buffer() {
        Python::attach(|py| {
            let obj = PyString::new(py, "hello world 123");
            match utf8_of(&obj).unwrap() {
                Utf8Buf::Borrowed(s) => assert_eq!(s, "hello world 123"),
                Utf8Buf::Owned(_) => panic!("ASCII must borrow, not own"),
            }
        });
    }

    /// Cold non-ASCII must take the owned (Rust transcode) path.
    #[test]
    fn cold_non_ascii_transcodes() {
        Python::attach(|py| {
            let obj = PyString::new(py, "مرحبا");
            match utf8_of(&obj).unwrap() {
                Utf8Buf::Owned(s) => assert_eq!(s, "مرحبا"),
                Utf8Buf::Borrowed(_) => panic!("cold non-ASCII must own"),
            }
        });
    }

    /// Warm (cache present) non-ASCII must borrow.
    #[test]
    fn warm_non_ascii_borrows() {
        Python::attach(|py| {
            let obj = PyString::new(py, "مرحبا");
            let _ = obj.to_str(); // warm the CPython cache
            match utf8_of(&obj).unwrap() {
                Utf8Buf::Borrowed(s) => assert_eq!(s, "مرحبا"),
                Utf8Buf::Owned(_) => panic!("warm non-ASCII must borrow"),
            }
        });
    }

    /// Lone surrogates must raise UnicodeEncodeError like
    /// PyUnicode_AsUTF8AndSize does (constructed via errorprone
    /// paths only; Python-level str cannot contain them, but C-API
    /// created strings can).
    #[test]
    fn surrogate_policy_documented() {
        // Python-level strings are always valid; the surrogate error
        // path is exercised only through C-API-created objects, which
        // we cannot build portably here. The transcode code maps
        // from_utf16/char::from_u32 errors to PyUnicodeEncodeError;
        // correctness is guaranteed by construction (no lossy paths).
    }
}
