//! Exception type exposed by the `rurex` module.

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::types::PyAny;

// Error: raised for pattern compilation errors; subclasses `Exception`,
// exactly like `re.error` (ADR-0004).
create_exception!(rurex, Error, pyo3::exceptions::PyException);

/// Convert a byte offset into a character index within `text`.
pub(crate) fn byte_offset_to_char(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    // Snap down to a char boundary, then count characters.
    let mut end = offset;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].chars().count()
}

/// Compute `(line, column)` for a character position the way CPython's
/// `re.error` does: line counts '\n' before pos plus one; column is the
/// distance past the last '\n' (pos + 1 when there is none).
pub(crate) fn line_col_text(text: &str, char_pos: usize) -> (usize, usize) {
    let mut lineno = 1;
    let mut last_nl = None;
    for (i, ch) in text.chars().enumerate().take(char_pos) {
        if ch == '\n' {
            lineno += 1;
            last_nl = Some(i);
        }
    }
    let colno = match last_nl {
        Some(i) => char_pos - i,
        None => char_pos + 1,
    };
    (lineno, colno)
}

/// Byte-index variant of [`line_col_text`], used for bytes patterns
/// (positions are byte offsets there, as in `re`).
pub(crate) fn line_col_bytes(pattern: &[u8], byte_pos: usize) -> (usize, usize) {
    let upto = &pattern[..pattern.len().min(byte_pos)];
    let lineno = upto.iter().filter(|&&b| b == b'\n').count() + 1;
    let colno = match upto.iter().rposition(|&b| b == b'\n') {
        Some(i) => byte_pos - i,
        None => byte_pos + 1,
    };
    (lineno, colno)
}

/// Build a `rurex.error` carrying `re.error`-style attributes:
/// `msg`, `pattern`, `pos`, `lineno`, `colno`.
///
/// The exception message mirrors CPython's formatting: `"{msg} at
/// position {pos}"`, with `(line L, column C)` inserted for multi-line
/// patterns. `None` position (engine-limit errors, e.g. compiled too
/// big) yields a bare message and `None` attributes.
pub(crate) fn make_error(
    py: Python<'_>,
    msg: &str,
    pattern: &Bound<'_, PyAny>,
    pos: Option<usize>,
    lineno: Option<usize>,
    colno: Option<usize>,
) -> PyResult<PyErr> {
    let mut text = msg.to_string();
    if let Some(p) = pos {
        text = format!("{msg} at position {p}");
        if lineno.is_some_and(|l| l > 1) {
            text = format!(
                "{msg} (line {}, column {}) at position {p}",
                lineno.unwrap(),
                colno.unwrap()
            );
        }
    }
    let err = Error::new_err(text);
    let value = err.value(py);
    value.setattr("msg", msg)?;
    value.setattr("pattern", pattern)?;
    value.setattr("pos", pos)?;
    value.setattr("lineno", lineno)?;
    value.setattr("colno", colno)?;
    Ok(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_to_char_offset() {
        assert_eq!(byte_offset_to_char("abc", 2), 2);
        assert_eq!(byte_offset_to_char("aé(", 3), 2);
        assert_eq!(byte_offset_to_char("é(", 2), 1);
        assert_eq!(byte_offset_to_char("ab", 9), 2); // clamped
    }

    #[test]
    fn line_col_single_line() {
        assert_eq!(line_col_text("(a", 2), (1, 3));
        assert_eq!(line_col_text("ab", 0), (1, 1));
    }

    #[test]
    fn line_col_multi_line() {
        // CPython: colno = pos - rfind('\n', 0, pos)
        assert_eq!(line_col_text("ab\ncd", 5), (2, 3));
        assert_eq!(line_col_text("ab\ncd", 3), (2, 1));
        assert_eq!(line_col_bytes(b"ab\ncd", 5), (2, 3));
    }
}
