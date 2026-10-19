//! rurex: Python bindings to the Rust `regex` crate.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyString};

mod error;
mod flags;
mod match_;
mod prefilter;
mod regex;
mod regexset;
mod retry;
mod template;
mod utf8;
mod vendor;

use match_::Match;
use regex::{FindIter, Regex, Scanner, substitute};
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

/// Bumped by `purge`, so every thread can drop its own last hit
/// without the cache reaching into another thread's storage.
static CACHE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// One-entry per-thread front for the module cache.
///
/// Module functions (`rurex.findall(pattern, ...)` and friends) are
/// call-dense: the same pattern text is used on every call, and the
/// shared cache charges a lock, a hash and a key allocation each time.
/// Comparing the pattern bytes against the previous hit by slice avoids
/// all three. A miss falls through to the shared cache and is then
/// recorded here; `purge` invalidates the entry through the generation
/// counter.
struct LastHit {
    generation: u64,
    bytes_mode: bool,
    flags: u32,
    pattern: Vec<u8>,
    rx: Py<Regex>,
}

fn last_hit() -> &'static std::thread::LocalKey<RefCell<Option<LastHit>>> {
    thread_local! {
        static LAST: RefCell<Option<LastHit>> = const { RefCell::new(None) };
    }
    &LAST
}

fn remember_hit(
    py: Python<'_>,
    generation: u64,
    bytes_mode: bool,
    flags: u32,
    pattern: &[u8],
    rx: &Py<Regex>,
) {
    last_hit().with(|slot| {
        *slot.borrow_mut() = Some(LastHit {
            generation,
            bytes_mode,
            flags,
            pattern: pattern.to_vec(),
            rx: rx.clone_ref(py),
        });
    });
}

/// Compile (or fetch from the module cache) a pattern for a
/// module-level convenience function. Mirrors CPython's `re._compile`:
/// FIFO eviction at 512 entries, keyed by pattern type, text, and
/// flags.
fn cached_compile(py: Python<'_>, pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Py<Regex>> {
    // Like re: an already-compiled pattern is returned as-is (flags
    // are rejected — they cannot be combined with a compiled pattern).
    if let Ok(rx) = pattern.extract::<PyRef<'_, Regex>>() {
        if flags != 0 {
            return Err(PyValueError::new_err(
                "cannot process flags argument with a compiled pattern",
            ));
        }
        return Ok(rx.into_pyobject(py).expect("infallible").unbind());
    }
    // Borrow the pattern text (zero-copy) before any cache work: the
    // fast path compares it against the thread's last hit by slice, and
    // only a miss pays for a key.
    let (bytes_mode, text): (bool, &[u8]) = if let Ok(s) = pattern.cast::<PyString>() {
        (
            false,
            s.to_str()
                .map_err(|_| PyTypeError::new_err("invalid str pattern"))?
                .as_bytes(),
        )
    } else if let Ok(b) = pattern.cast::<PyBytes>() {
        (true, b.as_bytes())
    } else {
        return Err(PyTypeError::new_err(format!(
            "pattern must be str or bytes, not {}",
            pattern.get_type()
        )));
    };

    let generation = CACHE_GENERATION.load(Ordering::Relaxed);
    let hit = last_hit().with(|slot| match &*slot.borrow() {
        Some(h)
            if h.generation == generation
                && h.bytes_mode == bytes_mode
                && h.flags == flags
                && h.pattern.as_slice() == text =>
        {
            Some(h.rx.clone_ref(py))
        }
        _ => None,
    });
    if let Some(rx) = hit {
        return Ok(rx);
    }

    let key: CacheKey = (bytes_mode, text.to_vec(), flags);
    {
        let cache = CACHE.lock().expect("pattern cache lock");
        if let Some(rx) = cache.map.get(&key) {
            let rx = rx.clone_ref(py);
            drop(cache);
            remember_hit(py, generation, bytes_mode, flags, text, &rx);
            return Ok(rx);
        }
    }
    let rx = Py::new(py, Regex::build(pattern, flags)?)?;
    {
        let mut cache = CACHE.lock().expect("pattern cache lock");
        if cache.map.len() >= MAX_CACHE {
            if let Some(oldest) = cache.order.pop_front() {
                cache.map.remove(&oldest);
            }
        }
        cache.map.insert(key.clone(), rx.clone_ref(py));
        cache.order.push_back(key);
    }
    remember_hit(py, generation, bytes_mode, flags, text, &rx);
    Ok(rx)
}

/// Escape all regex metacharacters in `text` (str or bytes, like
/// `re.escape`).
#[pyfunction]
fn escape<'py>(py: Python<'py>, string: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(s) = string.cast::<PyString>() {
        let escaped = escape_bytes(
            s.to_str()
                .map_err(|_| PyTypeError::new_err("invalid str argument"))?
                .as_bytes(),
        );
        Ok(PyString::new(py, &String::from_utf8_lossy(&escaped)).into_any())
    } else if let Ok(b) = string.cast::<PyBytes>() {
        Ok(PyBytes::new(py, &escape_bytes(b.as_bytes())).into_any())
    } else {
        Err(PyTypeError::new_err(format!(
            "escape() argument must be str or bytes, not {}",
            string.get_type()
        )))
    }
}

/// Prefix a backslash to every regex metacharacter, mirroring
/// CPython's `re.escape` byte-for-byte (the same special-character
/// set; non-metacharacters, including non-ASCII, pass through).
fn escape_bytes(bytes: &[u8]) -> Vec<u8> {
    const SPECIAL: &[u8] = b"()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c";
    let mut out = Vec::with_capacity(bytes.len() + 8);
    for &b in bytes {
        if SPECIAL.contains(&b) {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// Compile a regular expression into a `rurex.Pattern`. An already
/// compiled pattern is returned unchanged, like `re.compile`.
#[pyfunction]
#[pyo3(signature = (pattern, flags = 0))]
fn compile(py: Python<'_>, pattern: &Bound<'_, PyAny>, flags: u32) -> PyResult<Py<Regex>> {
    if let Ok(rx) = pattern.extract::<PyRef<'_, Regex>>() {
        if flags != 0 {
            return Err(PyValueError::new_err(
                "cannot process flags argument with a compiled pattern",
            ));
        }
        return Ok(rx.into_pyobject(py).expect("infallible").unbind());
    }
    Py::new(py, Regex::build(pattern, flags)?)
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
    CACHE_GENERATION.fetch_add(1, Ordering::Relaxed);
    CACHE.lock().expect("pattern cache lock").clear();
}

#[pyfunction]
#[pyo3(signature = (pattern, string, flags = 0))]
fn search<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(match Regex::search_from(&rx, py, string, 0, None)? {
        Some(m) => Some(Py::new(py, m)?),
        None => None,
    })
}

/// `re.prefixmatch` — the Python 3.15 preferred name for `match`.
#[pyfunction]
#[pyo3(signature = (pattern, string, flags = 0))]
fn prefixmatch<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(match Regex::match_from(&rx, py, string, 0, None)? {
        Some(m) => Some(Py::new(py, m)?),
        None => None,
    })
}

/// The historic name for `prefixmatch`, soft deprecated in CPython
/// 3.15 but kept for compatibility — exactly like `re`.
#[pyfunction]
#[pyo3(name = "match", signature = (pattern, string, flags = 0))]
fn py_match<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(match Regex::match_from(&rx, py, string, 0, None)? {
        Some(m) => Some(Py::new(py, m)?),
        None => None,
    })
}

#[pyfunction]
#[pyo3(signature = (pattern, string, flags = 0))]
fn fullmatch<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Option<Py<Match>>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(match Regex::fullmatch_from(&rx, py, string, 0, None)? {
        Some(m) => Some(Py::new(py, m)?),
        None => None,
    })
}

#[pyfunction]
#[pyo3(signature = (pattern, string, flags = 0))]
fn findall<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(Regex::findall_from(&rx, py, string, 0, None)?.into_any())
}

#[pyfunction]
#[pyo3(signature = (pattern, string, flags = 0))]
fn finditer(
    py: Python<'_>,
    pattern: &Bound<'_, PyAny>,
    string: &Bound<'_, PyAny>,
    flags: u32,
) -> PyResult<Py<FindIter>> {
    let rx = cached_compile(py, pattern, flags)?;
    let it = Regex::finditer_from(&rx, py, string, 0, None)?;
    Py::new(py, it)
}

#[pyfunction]
#[pyo3(signature = (pattern, string, maxsplit = 0, flags = 0))]
fn split<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    maxsplit: usize,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    Ok(Regex::split_from(&rx, py, string, maxsplit)?.into_any())
}

#[pyfunction]
#[pyo3(signature = (pattern, repl, string, count = 0, flags = 0))]
fn sub<'py>(
    py: Python<'py>,
    pattern: &Bound<'py, PyAny>,
    repl: &Bound<'py, PyAny>,
    string: &Bound<'py, PyAny>,
    count: usize,
    flags: u32,
) -> PyResult<Bound<'py, PyAny>> {
    let rx = cached_compile(py, pattern, flags)?;
    let (result, _) = substitute(&rx, py, repl, string, count)?;
    Ok(result.into_bound(py))
}

#[pyfunction]
#[pyo3(signature = (pattern, repl, string, count = 0, flags = 0))]
fn subn(
    py: Python<'_>,
    pattern: &Bound<'_, PyAny>,
    repl: &Bound<'_, PyAny>,
    string: &Bound<'_, PyAny>,
    count: usize,
    flags: u32,
) -> PyResult<(Py<PyAny>, usize)> {
    let rx = cached_compile(py, pattern, flags)?;
    substitute(&rx, py, repl, string, count)
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
    m.add_function(wrap_pyfunction!(py_match, m)?)?;
    m.add_function(wrap_pyfunction!(fullmatch, m)?)?;
    m.add_function(wrap_pyfunction!(findall, m)?)?;
    m.add_function(wrap_pyfunction!(finditer, m)?)?;
    m.add_function(wrap_pyfunction!(split, m)?)?;
    m.add_function(wrap_pyfunction!(sub, m)?)?;
    m.add_function(wrap_pyfunction!(subn, m)?)?;
    m.add_class::<Regex>()?;
    m.add_class::<Scanner>()?;
    m.add_class::<RegexSet>()?;
    m.add_class::<Match>()?;
    m.add_class::<FindIter>()?;
    m.add("error", m.py().get_type::<error::PatternError>())?;
    // CPython 3.13+ alias.
    m.add("PatternError", m.py().get_type::<error::PatternError>())?;

    // Flags as a `RegexFlag` IntFlag, mirroring `re` (Python 3.11+):
    // same members, same values, short names as aliases, and the same
    // module-prefixed repr (`str(rurex.I)` is `'rurex.IGNORECASE'`).
    // `LOCALE` and `DEBUG` exist as constants like in `re`; using
    // them still raises `ValueError` (see flags::translate). Rust
    // internals keep taking plain u32s — IntFlag members convert
    // transparently both ways.
    let py = m.py();
    let flag_globals = pyo3::types::PyDict::new(py);
    flag_globals.set_item("__name__", "rurex")?;
    flag_globals.set_item("IntFlag", py.import("enum")?.getattr("IntFlag")?)?;
    py.run(
        cr#"class RegexFlag(IntFlag):
    ASCII = 256
    IGNORECASE = 2
    LOCALE = 4
    UNICODE = 32
    MULTILINE = 8
    DOTALL = 16
    VERBOSE = 64
    DEBUG = 128

    A = ASCII
    I = IGNORECASE
    L = LOCALE
    M = MULTILINE
    S = DOTALL
    U = UNICODE
    X = VERBOSE

    def __repr__(self):
        module = 'rurex'
        if self._name_ is not None:
            return '|'.join(
                f'{module}.{n}' for n in self.name.split('|')
            )
        # Pre-3.11 enum machinery: composite values carry no computed
        # name -- decompose into canonical members instead.
        cls = type(self)
        value = self._value_
        names = [
            n for n, m in cls.__members__.items()
            if m._name_ == n and m._value_ and value & m._value_ == m._value_
        ]
        if not names:
            return f'{module}.RegexFlag({value!r})'
        return '|'.join(f'{module}.{n}' for n in names)

    __str__ = __repr__
    "#,
        Some(&flag_globals),
        None,
    )?;
    let flag_cls = flag_globals
        .get_item("RegexFlag")?
        .expect("RegexFlag defined above");
    m.add("RegexFlag", flag_cls.clone())?;
    for name in [
        "A",
        "ASCII",
        "I",
        "IGNORECASE",
        "L",
        "LOCALE",
        "M",
        "MULTILINE",
        "S",
        "DOTALL",
        "U",
        "UNICODE",
        "X",
        "VERBOSE",
        "DEBUG",
    ] {
        m.add(name, flag_cls.getattr(name)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::escape_bytes;

    #[test]
    fn escapes_metacharacters() {
        assert_eq!(escape_bytes(b"a.b*c"), br"a\.b\*c");
        assert_eq!(escape_bytes(b"plain"), b"plain");
        assert_eq!(escape_bytes(b"(a)"), br"\(a\)");
        // High bytes and non-metacharacters pass through untouched.
        assert_eq!(escape_bytes(b"\xff\xc3\xa9~"), b"\xff\xc3\xa9\\~");
        assert_eq!(escape_bytes(b"a b"), br"a\ b");
    }
}
