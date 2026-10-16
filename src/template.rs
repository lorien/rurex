//! `re`-style replacement templates (used by `sub`, `subn`, `expand`).
//!
//! Mirrors CPython's `re._parser.parse_template` semantics and messages,
//! operating on raw template bytes. Text-mode callers pass the template
//! as UTF-8 bytes; literal ranges only ever split at ASCII boundaries,
//! so converting them back with `from_utf8` is lossless.

use std::collections::HashMap;
use std::ops::Range;

/// One parsed piece of a template.
#[derive(Debug)]
pub(crate) enum Piece {
    /// Literal bytes, as a range into the original template.
    Lit(Range<usize>),
    /// A literal byte computed from a simple escape (`\n`, `\\`, ...).
    Byte(u8),
    /// A code point from an octal escape: UTF-8 encoded for `str`
    /// templates (like `re`), a raw byte for `bytes` templates.
    Char(u32),
    /// A group reference.
    Group(usize),
}

/// A parsed template.
#[derive(Debug)]
pub(crate) struct Template {
    pub(crate) pieces: Vec<Piece>,
}

/// A template error. `index_error` marks the `unknown group name` case,
/// which CPython raises as `IndexError` rather than `re.error`.
#[derive(Debug)]
pub(crate) struct TplError {
    pub msg: String,
    /// Byte position in the template (callers convert to char positions
    /// for text mode).
    pub pos: usize,
    pub index_error: bool,
}

impl TplError {
    fn error(msg: String, pos: usize) -> Self {
        TplError {
            msg,
            pos,
            index_error: false,
        }
    }
}

fn is_octal(b: u8) -> bool {
    b.is_ascii_digit() && b <= b'7'
}

/// Parse a template against a pattern's group count and name map.
pub(crate) fn parse(
    template: &[u8],
    ngroups: usize,
    names: &HashMap<&str, usize>,
) -> Result<Template, TplError> {
    let mut pieces = Vec::new();
    let mut lit_start = 0usize;
    let flush = |pieces: &mut Vec<Piece>, upto: usize, lit_start: usize| {
        if upto > lit_start {
            pieces.push(Piece::Lit(lit_start..upto));
        }
    };
    let group = |pieces: &mut Vec<Piece>, index: usize, pos: usize| -> Result<(), TplError> {
        if index > ngroups {
            return Err(TplError::error(
                format!("invalid group reference {index}"),
                pos + 1,
            ));
        }
        pieces.push(Piece::Group(index));
        Ok(())
    };

    let mut i = 0usize;
    while i < template.len() {
        if template[i] != b'\\' {
            i += 1;
            continue;
        }
        // A lone trailing backslash is a literal backslash.
        if i + 1 >= template.len() {
            flush(&mut pieces, i, lit_start);
            pieces.push(Piece::Byte(b'\\'));
            return Ok(Template { pieces });
        }
        let c = template[i + 1];
        let escape_pos = i;
        match c {
            b'g' => {
                if i + 2 >= template.len() || template[i + 2] != b'<' {
                    return Err(TplError::error("missing <".to_string(), escape_pos + 2));
                }
                let Some(rel) = template[i + 3..].iter().position(|&b| b == b'>') else {
                    return Err(TplError::error(
                        "missing >, unterminated name".to_string(),
                        i + 3,
                    ));
                };
                let content = &template[i + 3..i + 3 + rel];
                let after = i + 3 + rel + 1;
                if !content.is_empty() && content.iter().all(u8::is_ascii_digit) {
                    let index: usize = std::str::from_utf8(content)
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(usize::MAX);
                    if index >= 100 {
                        // CPython positions \g<...> reference errors
                        // at the first character inside '<'.
                        return Err(TplError::error(
                            format!("invalid group reference {index}"),
                            i + 3,
                        ));
                    }
                    flush(&mut pieces, i, lit_start);
                    group(&mut pieces, index, escape_pos)?;
                } else {
                    let name = String::from_utf8_lossy(content).into_owned();
                    match names.get(name.as_str()) {
                        Some(&index) => {
                            flush(&mut pieces, i, lit_start);
                            group(&mut pieces, index, escape_pos)?;
                        }
                        None => {
                            return Err(TplError {
                                msg: format!("unknown group name '{name}'"),
                                pos: i + 3,
                                index_error: true,
                            });
                        }
                    }
                }
                lit_start = after;
                i = after;
            }
            b'0' => {
                // Octal escape: \0 plus up to two more octal digits.
                let mut value: u32 = 0;
                let mut j = i + 1;
                while j < template.len() && is_octal(template[j]) && j < i + 4 {
                    value = value * 8 + (template[j] - b'0') as u32;
                    j += 1;
                }
                if value > 0o377 {
                    let lit = String::from_utf8_lossy(&template[i + 1..j]).into_owned();
                    return Err(TplError::error(
                        format!("octal escape value \\{lit} outside of range 0-0o377"),
                        escape_pos,
                    ));
                }
                flush(&mut pieces, i, lit_start);
                pieces.push(Piece::Char(value));
                lit_start = j;
                i = j;
            }
            b'1'..=b'9' => {
                let mut digits_end = i + 2;
                if digits_end < template.len() && template[digits_end].is_ascii_digit() {
                    digits_end += 1;
                    // Three-digit octal escape, e.g. \141 == 'a'.
                    if is_octal(c)
                        && is_octal(template[digits_end - 1])
                        && digits_end < template.len()
                        && is_octal(template[digits_end])
                    {
                        let value = u32::from(c - b'0') * 64
                            + u32::from(template[digits_end - 1] - b'0') * 8
                            + u32::from(template[digits_end] - b'0');
                        if value > 0o377 {
                            let lit =
                                String::from_utf8_lossy(&template[i + 1..=digits_end]).into_owned();
                            return Err(TplError::error(
                                format!("octal escape value \\{lit} outside of range 0-0o377"),
                                escape_pos,
                            ));
                        }
                        flush(&mut pieces, i, lit_start);
                        pieces.push(Piece::Char(value));
                        lit_start = digits_end + 1;
                        i = digits_end + 1;
                        continue;
                    }
                }
                let digits = std::str::from_utf8(&template[i + 1..digits_end]).unwrap_or("");
                let index: usize = digits.parse().unwrap_or(usize::MAX);
                flush(&mut pieces, i, lit_start);
                group(&mut pieces, index, escape_pos)?;
                lit_start = digits_end;
                i = digits_end;
            }
            b'a' | b'b' | b'f' | b'n' | b'r' | b't' | b'v' | b'\\' => {
                let byte = match c {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b'f' => 0x0c,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'v' => 0x0b,
                    _ => b'\\',
                };
                flush(&mut pieces, i, lit_start);
                pieces.push(Piece::Byte(byte));
                lit_start = i + 2;
                i += 2;
            }
            c if c.is_ascii_alphabetic() => {
                return Err(TplError::error(
                    format!("bad escape \\{}", c as char),
                    escape_pos,
                ));
            }
            _ => {
                // Unknown non-letter escape: literal backslash + char.
                i += 2;
            }
        }
    }
    flush(&mut pieces, template.len(), lit_start);
    Ok(Template { pieces })
}

/// Expand a parsed template against match spans (byte units) over
/// `haystack`, appending to `out`.
pub(crate) fn expand(
    template: &Template,
    template_bytes: &[u8],
    spans: &[Option<(usize, usize)>],
    haystack: &[u8],
    text_mode: bool,
    out: &mut Vec<u8>,
) {
    for piece in &template.pieces {
        match piece {
            Piece::Lit(range) => out.extend_from_slice(&template_bytes[range.clone()]),
            Piece::Byte(b) => out.push(*b),
            Piece::Char(v) => {
                if text_mode {
                    let mut buf = [0u8; 4];
                    if let Some(ch) = char::from_u32(*v) {
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                } else {
                    out.push(*v as u8);
                }
            }
            Piece::Group(index) => {
                if let Some((start, end)) = spans.get(*index).copied().flatten() {
                    out.extend_from_slice(&haystack[start..end]);
                }
            }
        }
    }
}

/// Build a name-to-index map from an index-to-name vector.
pub(crate) fn names_map(names: &[Option<String>]) -> HashMap<&str, usize> {
    names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.as_deref().map(|n| (n, i)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(t: &str, ngroups: usize) -> Result<Template, TplError> {
        parse(t.as_bytes(), ngroups, &HashMap::new())
    }

    #[test]
    fn plain_literal() {
        let t = parse_str("hello", 0).unwrap();
        assert_eq!(t.pieces.len(), 1);
    }

    #[test]
    fn group_refs() {
        let t = parse_str(r"\1 and \g<2>", 2).unwrap();
        assert_eq!(t.pieces.len(), 3);
        assert!(matches!(t.pieces[0], Piece::Group(1)));
        assert!(matches!(t.pieces[2], Piece::Group(2)));
    }

    #[test]
    fn invalid_references() {
        assert_eq!(
            parse_str(r"\2", 1).unwrap_err().msg,
            "invalid group reference 2"
        );
        assert_eq!(
            parse_str(r"\10", 2).unwrap_err().msg,
            "invalid group reference 10"
        );
        assert_eq!(
            parse_str(r"\g<100>", 0).unwrap_err().msg,
            "invalid group reference 100"
        );
    }

    #[test]
    fn bad_escapes() {
        assert_eq!(parse_str(r"\q", 0).unwrap_err().msg, r"bad escape \q");
        assert!(!parse_str(r"\q", 0).unwrap_err().index_error);
        // Non-letter escapes stay literal.
        assert!(parse_str(r"\.", 0).is_ok());
    }

    #[test]
    fn known_escapes_and_octal() {
        let t = parse_str(r"\n\t\141\0", 0).unwrap();
        assert_eq!(t.pieces.len(), 4);
    }

    #[test]
    fn unterminated_name() {
        assert_eq!(
            parse_str(r"\g<1", 1).unwrap_err().msg,
            "missing >, unterminated name"
        );
        assert_eq!(parse_str(r"\g", 1).unwrap_err().msg, "missing <");
    }

    #[test]
    fn unknown_group_name_is_index_error() {
        let names = HashMap::from([("x", 1usize)]);
        let err = parse(b"\\g<y>", 1, &names).unwrap_err();
        assert!(err.index_error);
        assert_eq!(err.msg, "unknown group name 'y'");
    }

    #[test]
    fn expansion_pieces() {
        let names = HashMap::from([("x", 1usize)]);
        let t = parse(b"a\\g<x>b\\1", 1, &names).unwrap();
        let spans = vec![Some((0, 4)), Some((1, 3))];
        let hay = b"wxyzw";
        let mut out = Vec::new();
        expand(&t, b"a\\g<x>b\\1", &spans, hay, true, &mut out);
        assert_eq!(out, b"axybxy");
    }
}
