//! Flag constants and their translation to engine builder options.
//!
//! Flag bits are value-compatible with the stdlib `re` module, so users
//! can pass `re.IGNORECASE` (and friends) directly to `rurex.compile`
//! (ADR-0004, revised 2026-10-15). Bits the engine cannot honor are
//! rejected loudly, mirroring CPython's `re._parser.fix_flags` messages.

/// Case-insensitive matching (`re.IGNORECASE`).
pub const IGNORECASE: u32 = 2;
/// `^`/`$` match at line boundaries (`re.MULTILINE`).
pub const MULTILINE: u32 = 8;
/// `.` matches newlines (`re.DOTALL`).
pub const DOTALL: u32 = 16;
/// Unicode matching (`re.UNICODE`): no-op for `str`, error for `bytes`.
pub const UNICODE: u32 = 32;
/// Whitespace and `#` comments ignored in the pattern (`re.VERBOSE`).
pub const VERBOSE: u32 = 64;
/// ASCII-only classes and case folding (`re.ASCII`).
pub const ASCII: u32 = 256;

/// Engine configuration derived from flag bits.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
    pub ignore_whitespace: bool,
    pub unicode: bool,
    /// The value reported from `Regex.flags`, mirroring CPython's
    /// `fix_flags` (UNICODE is forced on for str patterns without ASCII).
    pub stored_flags: u32,
}

/// Translate flag bits into an engine [`Config`].
///
/// `bytes_mode` selects the pattern type (`bytes` versus `str`).
/// Returns a `ValueError`-bound message on unsupported or contradictory
/// flags; the messages match CPython's where CPython has them.
pub fn translate(flags: u32, bytes_mode: bool) -> Result<Config, String> {
    // Bits that exist in `re` but are not supported here or by the
    // engine. TEMPLATE (1) was removed from `re` in Python 3.13.
    for (bit, name) in [(1u32, "TEMPLATE"), (128, "DEBUG")] {
        if flags & bit != 0 {
            return Err(format!("unsupported flag: {name}"));
        }
    }
    if bytes_mode {
        if flags & UNICODE != 0 {
            return Err("cannot use UNICODE flag with a bytes pattern".to_string());
        }
        if flags & 4 != 0 {
            return Err("LOCALE is not supported by the rurex engine".to_string());
        }
    } else {
        if flags & 4 != 0 {
            return Err("cannot use LOCALE flag with a str pattern".to_string());
        }
        if flags & (ASCII | UNICODE) == (ASCII | UNICODE) {
            return Err("ASCII and UNICODE flags are incompatible".to_string());
        }
    }
    let unknown = flags & !(IGNORECASE | MULTILINE | DOTALL | UNICODE | VERBOSE | ASCII);
    if unknown != 0 {
        return Err(format!("unknown flag bits: 0x{unknown:x}"));
    }

    // Bytes patterns use the engine's non-Unicode mode so `\w` and
    // friends are ASCII-only single bytes, exactly like `re`; `(?u:...)`
    // groups in the pattern can still opt regions back into UTF-8-aware
    // matching.
    let unicode = !bytes_mode && flags & ASCII == 0;
    Ok(Config {
        case_insensitive: flags & IGNORECASE != 0,
        multi_line: flags & MULTILINE != 0,
        dot_matches_new_line: flags & DOTALL != 0,
        ignore_whitespace: flags & VERBOSE != 0,
        unicode,
        stored_flags: if unicode { flags | UNICODE } else { flags },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_defaults_to_unicode() {
        let cfg = translate(0, false).unwrap();
        assert!(cfg.unicode);
        assert_eq!(cfg.stored_flags, UNICODE);
        assert!(!cfg.case_insensitive && !cfg.multi_line);
    }

    #[test]
    fn bytes_defaults_to_non_unicode() {
        let cfg = translate(0, true).unwrap();
        assert!(!cfg.unicode);
        assert_eq!(cfg.stored_flags, 0);
    }

    #[test]
    fn ascii_disables_unicode_for_str() {
        let cfg = translate(ASCII, false).unwrap();
        assert!(!cfg.unicode);
        assert_eq!(cfg.stored_flags, ASCII);
    }

    #[test]
    fn ascii_is_noop_for_bytes() {
        let cfg = translate(ASCII, true).unwrap();
        assert!(!cfg.unicode);
    }

    #[test]
    fn mode_flags_map() {
        let cfg = translate(IGNORECASE | MULTILINE | DOTALL | VERBOSE, false).unwrap();
        assert!(cfg.case_insensitive && cfg.multi_line);
        assert!(cfg.dot_matches_new_line && cfg.ignore_whitespace);
    }

    #[test]
    fn unicode_flag_is_noop_for_str() {
        let cfg = translate(UNICODE, false).unwrap();
        assert!(cfg.unicode);
    }

    #[test]
    fn cpython_compatible_rejections() {
        assert_eq!(
            translate(UNICODE, true).err().unwrap(),
            "cannot use UNICODE flag with a bytes pattern"
        );
        assert_eq!(
            translate(4, false).err().unwrap(),
            "cannot use LOCALE flag with a str pattern"
        );
        assert_eq!(
            translate(ASCII | UNICODE, false).err().unwrap(),
            "ASCII and UNICODE flags are incompatible"
        );
    }

    #[test]
    fn engine_and_unknown_rejections() {
        assert_eq!(
            translate(4, true).err().unwrap(),
            "LOCALE is not supported by the rurex engine"
        );
        assert_eq!(
            translate(1, false).err().unwrap(),
            "unsupported flag: TEMPLATE"
        );
        assert_eq!(
            translate(128, false).err().unwrap(),
            "unsupported flag: DEBUG"
        );
        assert_eq!(
            translate(1 << 12, false).err().unwrap(),
            "unknown flag bits: 0x1000"
        );
    }
}

/// Scan pattern text (skipping escapes and character classes) and:
///
/// - fold leading global inline groups `(?imsxa)` into extra flag
///   bits (CPython reports them via `Pattern.flags`) — the engine
///   handles `imsx` natively; a global `(?a)` is stripped and turned
///   into the `ASCII` bit, which the builder cannot see as text;
/// - rewrite scoped `(?a:...)` groups to the engine's `(?-u:...)`
///   (same shape, non-capturing, ASCII semantics inside).
///
/// Byte-for-byte, positions are preserved except for stripped global
/// `(?a)` groups.
pub fn inline_adjust(text: &str, verbose_arg: bool) -> Result<(String, u32), usize> {
    let b = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut extra: u32 = 0;
    let mut leading = true;
    let mut i = 0usize;

    const FLAG_CHARS: &[u8] = b"aimsxuUL-";

    // In verbose mode, leading whitespace and `#` comments are
    // transparent for the "flags at the start" rule (CPython: both
    // `(?x) a` and ` (?x) a` under VERBOSE compile).
    let verbose_now = |extra: u32| verbose_arg || extra & VERBOSE != 0;

    while i < b.len() {
        let c = b[i];
        if leading && verbose_now(extra) && (c.is_ascii_whitespace() || c == b'#') {
            if c == b'#' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            i += 1;
            continue;
        }
        if c == b'\\' {
            leading = false;
            out.push(c);
            if i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == b'[' {
            leading = false;
            let start = i;
            i += 1;
            if i < b.len() && b[i] == b'^' {
                i += 1;
            }
            if i < b.len() && b[i] == b']' {
                i += 1;
            }
            while i < b.len() && b[i] != b']' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.extend_from_slice(&b[start..i]);
            continue;
        }
        if c == b'(' && i + 1 < b.len() && b[i + 1] == b'?' {
            // Try to parse `(?flags)` or `(?flags:` with flags from
            // FLAG_CHARS only; anything else is copied verbatim.
            let mut j = i + 2;
            while j < b.len() && FLAG_CHARS.contains(&b[j]) {
                j += 1;
            }
            if j > i + 2 && j < b.len() && (b[j] == b')' || b[j] == b':') {
                let flags = &b[i + 2..j];
                let scoped = b[j] == b':';
                let all_global = flags
                    .iter()
                    .all(|f| matches!(f, b'a' | b'i' | b'm' | b's' | b'x' | b'u' | b'L'));
                if !scoped && all_global && !leading {
                    // CPython 3.11+: global flags must lead the pattern.
                    return Err(i);
                }
                if !scoped
                    && leading
                    && flags
                        .iter()
                        .all(|f| matches!(f, b'a' | b'i' | b'm' | b's' | b'x'))
                {
                    // Leading global group: fold into flags, strip.
                    for &f in flags {
                        extra |= match f {
                            b'a' => ASCII,
                            b'i' => IGNORECASE,
                            b'm' => MULTILINE,
                            b's' => DOTALL,
                            b'x' => VERBOSE,
                            _ => 0,
                        };
                    }
                    i = j + 1;
                    continue;
                }
                if scoped && flags.len() == 1 && flags[0] == b'a' {
                    // `(?a:` -> `(?-u:` (same length).
                    out.extend_from_slice(b"(?-u:");
                    i = j + 1;
                    continue;
                }
            }
            leading = false;
            out.push(c);
            i += 1;
            continue;
        }
        if c != b'(' {
            leading = false;
        }
        out.push(c);
        i += 1;
    }
    Ok((
        String::from_utf8(out).unwrap_or_else(|_| text.to_string()),
        extra,
    ))
}

/// Reject stacked quantifiers the way CPython does (`a**`, `a*{1,2}`
/// → `multiple repeat`), which the engine would silently accept as
/// nested repetition. Returns the byte offset of the offending second
/// operator. One `?` (lazy) or `+` (possessive) modifier after a
/// quantifier is legal; `verbose` skips whitespace and `#` comments.
pub fn multiple_repeat(pattern: &str, verbose: bool) -> Option<usize> {
    let b = pattern.as_bytes();
    let mut i = 0usize;
    // quantified: a quantifier (and its optional single modifier) was
    // just consumed; atom: a literal/escape/class/group was consumed.
    let mut quantified = false;
    let mut modifier_used = false;
    let mut atom_seen = false;
    while i < b.len() {
        let c = b[i];
        if verbose && (c == b'#' || c.is_ascii_whitespace()) {
            if c == b'#' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            } else {
                i += 1;
            }
            continue;
        }
        if c == b'\\' {
            i += 2;
            (quantified, modifier_used) = (false, false);
            atom_seen = true;
            continue;
        }
        if c == b'[' {
            let start = i;
            i += 1;
            if i < b.len() && b[i] == b'^' {
                i += 1;
            }
            if i < b.len() && b[i] == b']' {
                i += 1;
            }
            while i < b.len() && b[i] != b']' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            let _ = start;
            (quantified, modifier_used) = (false, false);
            atom_seen = true;
            continue;
        }
        let is_counted = c == b'{' && b.get(i + 1).is_some_and(u8::is_ascii_digit);
        if c == b'*' || c == b'+' || c == b'?' || is_counted {
            if quantified && (!matches!(c, b'?' | b'+') || modifier_used) {
                return Some(i);
            }
            if quantified && matches!(c, b'?' | b'+') {
                // The single lazy/possessive modifier.
                modifier_used = true;
                i += 1;
                continue;
            }
            if atom_seen {
                quantified = true;
                modifier_used = false;
            }
            // else: a leading operator — the engine's own
            // `nothing to repeat` reports it; not a stacking error.
            if is_counted {
                i += 1;
                while i < b.len() && (b[i].is_ascii_digit() || matches!(b[i], b',' | b'}')) {
                    i += 1;
                }
            } else {
                i += 1;
            }
            continue;
        }
        // A literal or group character: `)` and `|` leave an atom
        // boundary, everything else is (part of) an atom.
        (quantified, modifier_used) = (false, false);
        atom_seen = c != b'(' && c != b'|';
        i += 1;
    }
    None
}

/// Validate `(?P<name>` group names the way CPython does: the engine
/// accepts names the `re` parser rejects (e.g. `(?P<a.>)`). Returns
/// the CPython message and byte offset of the first bad name.
pub fn group_name_check(pattern: &str) -> Option<(String, usize)> {
    let bytes = pattern.as_bytes();
    // Snap forward to a char boundary (escapes may skip into a
    // multi-byte character).
    let snap = |i: usize| -> usize {
        let mut i = i;
        while i < pattern.len() && !pattern.is_char_boundary(i) {
            i += 1;
        }
        i
    };
    let mut i = 0usize;
    while i + 4 < bytes.len() {
        if bytes[i] == b'\\' {
            i = snap(i + 2);
            continue;
        }
        if bytes[i] == b'[' {
            while i < bytes.len() && bytes[i] != b']' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = snap(i + 1);
            continue;
        }
        if bytes[i] == b'(' && pattern[i..].starts_with("(?P<") {
            let name_start = i + 4;
            // Only fully-closed, well-delimited names are checked
            // here; unterminated or paren-cut names keep the engine's
            // error path (mapped separately).
            let Some(end) = pattern[name_start..].find('>').map(|e| name_start + e) else {
                i = name_start;
                continue;
            };
            let name = &pattern[name_start..end];
            let valid = !name.is_empty()
                && name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_alphanumeric() || c == '_');
            if !valid && !name.is_empty() {
                return Some((format!("bad character in group name '{name}'"), name_start));
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    None
}
