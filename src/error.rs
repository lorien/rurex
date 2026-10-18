//! Exception type exposed by the `rurex` module.

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::types::PyAny;

// PatternError: raised for pattern compilation errors; subclasses
// `Exception`, the class name mirroring `re.PatternError` (Python 3.13+);
// the module-level `error` attribute is the legacy alias, like `re.error`.
// exactly like `re.error` (ADR-0004).
create_exception!(rurex, PatternError, pyo3::exceptions::PyException);

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
    let err = PatternError::new_err(text);
    let value = err.value(py);
    value.setattr("msg", msg)?;
    value.setattr("pattern", pattern)?;
    value.setattr("pos", pos)?;
    value.setattr("lineno", lineno)?;
    value.setattr("colno", colno)?;
    Ok(err)
}

/// Rewrite a `regex-syntax` parse error to the message and byte offset
/// CPython's `re` reports, for the kinds where they differ. Returns
/// `None` to keep the engine's own wording. `pattern` is the (already
/// inline-flag-adjusted) source the engine sees.
pub(crate) fn cppythonize_ast(
    kind: &regex_syntax::ast::ErrorKind,
    span: &regex_syntax::ast::Span,
    pattern: &str,
) -> Option<(String, usize)> {
    use regex_syntax::ast::ErrorKind as K;
    let at = span.start.offset;
    let rest = &pattern[at.min(pattern.len())..];
    match kind {
        K::ClassUnclosed => Some(("unterminated character set".to_string(), at)),
        K::ClassRangeInvalid | K::ClassRangeLiteral => {
            let (msg, pos) = bad_character_range(pattern, at);
            Some((msg, pos))
        }
        K::RepetitionMissing => Some(("nothing to repeat".to_string(), at)),
        K::EscapeUnexpectedEof => {
            // A trailing `\x` (EOF before two hex digits) is
            // "incomplete escape \x"; a bare trailing backslash is
            // "bad escape (end of pattern)".
            let head = &pattern[..at.min(pattern.len())];
            if head.len() >= 2 && head.ends_with('x') && head[..head.len() - 1].ends_with('\\') {
                Some(("incomplete escape \\x".to_string(), at - 2))
            } else {
                Some(("bad escape (end of pattern)".to_string(), at))
            }
        }
        K::EscapeHexInvalidDigit | K::EscapeHexInvalid => {
            // CPython: `\x` not followed by two hex digits is
            // "incomplete escape \x" at the backslash.
            let bs = pattern[..at.min(pattern.len())].rfind('\\').unwrap_or(at);
            Some(("incomplete escape \\x".to_string(), bs))
        }
        K::GroupUnclosed => {
            if rest.starts_with("(?") && rest.len() == 2 {
                Some(("unexpected end of pattern".to_string(), at + 2))
            } else {
                Some(("missing ), unterminated subpattern".to_string(), at))
            }
        }
        K::GroupUnopened => Some(("unbalanced parenthesis".to_string(), at)),
        K::FlagUnrecognized
        | K::GroupNameInvalid
        | K::GroupNameEmpty
        | K::GroupNameUnexpectedEof => group_name_error(kind, pattern, at),
        K::GroupNameDuplicate { .. } => {
            let (msg, pos) = group_redefinition(pattern, at);
            Some((msg, pos))
        }
        _ => None,
    }
}

/// `bad character range a-b`: CPython points at the first range item
/// inside the class and quotes the range text up to the closing `]`.
fn bad_character_range(pattern: &str, err_at: usize) -> (String, usize) {
    let open = pattern[..err_at.min(pattern.len())].rfind('[');
    let (start, close) = match open {
        Some(i) => {
            // The close ']' is the first unescaped one (a leading ']'
            // is a literal, like in CPython).
            let mut j = i + 1;
            if j < pattern.len() && pattern.as_bytes()[j] == b'^' {
                j += 1;
            }
            if j < pattern.len() && pattern.as_bytes()[j] == b']' {
                j += 1;
            }
            while j < pattern.len() && pattern.as_bytes()[j] != b']' {
                if pattern.as_bytes()[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            (i + 1, j)
        }
        None => (err_at, pattern.len()),
    };
    let text = &pattern[start..close];
    (format!("bad character range {text}"), start)
}

/// The `(?P<...>` / `(?P=...` / `(?X` family, where CPython's messages
/// depend on the surrounding extension rather than the offending char.
fn group_name_error(
    kind: &regex_syntax::ast::ErrorKind,
    pattern: &str,
    at: usize,
) -> Option<(String, usize)> {
    use regex_syntax::ast::ErrorKind as K;
    let hay = &pattern[..at.min(pattern.len())];
    match kind {
        K::GroupNameEmpty => {
            let p = hay.rfind("(?P<").map(|i| i + 4).unwrap_or(at);
            Some(("missing group name".to_string(), p))
        }
        K::GroupNameUnexpectedEof => {
            // `(?P<foo` — "missing >, unterminated name" at the name;
            // `(?P<` with no name at all — "missing group name".
            let p = hay.rfind("(?P<").map(|i| i + 4).unwrap_or(at);
            let name = &pattern[p.min(pattern.len())..];
            if name.is_empty() {
                Some(("missing group name".to_string(), p))
            } else {
                Some(("missing >, unterminated name".to_string(), p))
            }
        }
        K::GroupNameInvalid | K::FlagUnrecognized => {
            // The error span points at the offending character; the
            // extension group starts at the last "(?" before it.
            let gi = hay.rfind("(?")?;
            let tail = &pattern[gi..];
            if tail.starts_with("(?P<") || tail.starts_with("(?P=") {
                // (?P<bad-name>…): the name runs to the next '>' or
                // ')'; (?P=bad-name…): to the next ')'.
                let name_part = &tail[4..];
                let end = if tail.starts_with("(?P=") {
                    name_part.find(')').unwrap_or(name_part.len())
                } else {
                    name_part.find(['>', ')']).unwrap_or(name_part.len())
                };
                let name = &name_part[..end];
                let is_def = tail.starts_with("(?P<");
                if name.is_empty() {
                    // `(?P<>)` — "missing group name"; `(?P<)` — the
                    // '>' itself is missing.
                    if name_part.starts_with('>') {
                        return Some(("missing group name".to_string(), gi + 4));
                    }
                    if is_def {
                        return Some(("missing >, unterminated name".to_string(), gi + 4));
                    }
                    return Some(("missing group name".to_string(), gi + 4));
                }
                if is_def {
                    return Some((format!("bad character in group name '{name}'"), gi + 4));
                }
                // (?P=name): referring to a still-open group, a bad
                // name, an unknown group, or an unterminated ref.
                if open_group_named(pattern, gi, name) {
                    return Some(("cannot refer to an open group".to_string(), gi + 4));
                }
                if !is_identifier(name) {
                    return Some((format!("bad character in group name '{name}'"), gi + 4));
                }
                if !pattern_contains_def(pattern, name) {
                    return Some((format!("unknown group name '{name}'"), gi + 4));
                }
                if name_part.find(')').is_none() {
                    // No ')' ever closes the reference.
                    return Some(("missing ), unterminated name".to_string(), gi + 4));
                }
                // A well-formed `(?P=name)` the engine cannot honor
                // (backreference): keep the engine's own error.
                return None;
            }
            if tail.starts_with("(?#") {
                return Some(("missing ), unterminated comment".to_string(), gi));
            }
            // `(?Z`, `(?Px`, `(?<(` …: unknown extension. CPython
            // quotes '?' plus the word characters; for the two-char
            // lookbehind-ish prefixes (`<`, `'`) it quotes exactly two
            // characters.
            let after = &tail[2..];
            let mut chars = after.chars();
            let word = match chars.next() {
                Some(c @ ('<' | '\'')) => {
                    let two: String = std::iter::once(c).chain(chars.next()).collect();
                    two
                }
                Some(c) => {
                    // Otherwise '?' plus one more word character only
                    // when the first is alphabetic ("?Px", not
                    // "?Pxy", not "?{1").
                    let two: String = std::iter::once(c)
                        .chain(after[1..].chars().next().filter(|c2| {
                            c.is_ascii_alphabetic() && (c2.is_ascii_alphanumeric() || *c2 == '_')
                        }))
                        .collect();
                    two
                }
                None => String::new(),
            };
            Some((format!("unknown extension ?{word}"), gi + 1))
        }
        _ => None,
    }
}

/// `redefinition of group name 'x' as group 2; was group 1`: counts
/// capture groups by a textual pre-scan (`(?P<n>` opens at its `(`).
fn group_redefinition(pattern: &str, at: usize) -> (String, usize) {
    let bytes = pattern.as_bytes();
    let mut group_no = 0usize;
    let mut first: Option<(&str, usize)> = None;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if pattern[i..].starts_with("(?P<") {
            group_no += 1;
            let name_start = i + 4;
            let end = pattern[name_start..]
                .find('>')
                .map(|e| name_start + e)
                .unwrap_or(pattern.len());
            let name = &pattern[name_start..end];
            if i + 4 == at {
                // This is the redefinition (the span points at the
                // duplicated name, i.e. i + 4).
                let was = first
                    .filter(|(n, _)| *n == name)
                    .map(|(_, g)| g)
                    .unwrap_or(1);
                return (
                    format!(
                        "redefinition of group name '{name}' as group {group_no}; was group {was}"
                    ),
                    name_start,
                );
            }
            if first.is_none() {
                first = Some((name, group_no));
            }
            i = end + 1;
            continue;
        }
        if bytes[i] == b'(' && bytes.get(i + 1) != Some(&b'?') {
            group_no += 1;
        }
        i += 1;
    }
    ("duplicate capture group name".to_string(), at)
}

/// Translate-kind rewrites (post-parse semantic errors).
pub(crate) fn cppythonize_hir(
    kind: &regex_syntax::hir::ErrorKind,
    span: &regex_syntax::ast::Span,
    _pattern: &str,
) -> Option<(String, usize)> {
    use regex_syntax::hir::ErrorKind as K;
    let at = span.start.offset;
    match kind {
        K::UnicodePropertyNotFound => Some(("bad escape \\p".to_string(), at)),
        _ => None,
    }
}

/// True when `name` is a Python identifier (group-name rules).
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// True when `(?P<name>` is defined somewhere in `pattern`.
fn pattern_contains_def(pattern: &str, name: &str) -> bool {
    pattern.contains(&format!("(?P<{name}>"))
}

/// If a `(?P<name>` group is open (parenthetically) at byte offset
/// `at`: a `(?P=name)` may not refer to it (CPython: "cannot refer
/// to an open group").
fn open_group_named(pattern: &str, at: usize, name: &str) -> bool {
    let bytes = pattern.as_bytes();
    let mut depth: Vec<&str> = Vec::new();
    let mut i = 0usize;
    while i < at && i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'[' => {
                while i < at && i < bytes.len() && bytes[i] != b']' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'(' => {
                if pattern[i..].starts_with("(?P<") {
                    let end = pattern[i + 4..]
                        .find('>')
                        .map(|e| i + 4 + e)
                        .unwrap_or(pattern.len());
                    let nm = &pattern[(i + 4).min(at)..end.min(at)];
                    depth.push(nm);
                    i = end;
                } else {
                    depth.push("");
                }
            }
            b')' => {
                depth.pop();
            }
            _ => {}
        }
        i += 1;
    }
    depth.iter().any(|n| *n == name && !name.is_empty())
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
