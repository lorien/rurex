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
