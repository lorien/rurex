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
pub fn inline_adjust(text: &str) -> (String, u32) {
    let b = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut extra: u32 = 0;
    let mut leading = true;
    let mut i = 0usize;

    const FLAG_CHARS: &[u8] = b"aimsxuUL-";

    while i < b.len() {
        let c = b[i];
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
    (
        String::from_utf8(out).unwrap_or_else(|_| text.to_string()),
        extra,
    )
}
