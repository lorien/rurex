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

/// The largest group number CPython accepts (`_sre.MAXGROUPS`), so a
/// reference at or above it is invalid even in a pattern with no groups.
const MAXGROUPS: u64 = 1_073_741_823;

/// One open group of the conditional scan below.
enum Frame {
    /// A plain group: capture or `(?:`-style.
    Group,
    /// A conditional's body, waiting for its closing `)`. `at` is the
    /// offset of its `(?(` and `pipes` counts the top-level `|` seen in
    /// it (a conditional takes at most two branches).
    Conditional { at: usize, pipes: usize },
}

/// Do the textual checks CPython's parser performs on conditionals,
/// `(?(`ref`)`yes`|`no`)`, and report the first violation as
/// (message, byte offset) — the same shape as `group_name_check`.
///
/// rurex does not support conditionals (the engine has none), so a
/// *valid* conditional still fails to compile; without this scan every
/// one of them reported the engine's generic
/// "unknown extension ?(", including the ones CPython rejects with a
/// specific message and position (`test_re_groupref_exists_errors`,
/// `test_re_groupref_overflow`). The scan only ever rejects patterns
/// CPython rejects, and only for the reason CPython gives first: it runs
/// left to right, skipping escapes and character classes, and validates
/// numeric references against the final group count (CPython counts the
/// implicit group 0, so `N` is valid when `N <= groups`).
pub fn conditional_check(pattern: &str) -> Option<(String, usize)> {
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
    let mut groups = 0usize;
    let mut names: Vec<&str> = Vec::new();
    // Numeric references are checked when the whole pattern has been
    // scanned, because CPython validates them against the final count.
    let mut digit_refs: Vec<(u64, usize)> = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i = snap(i + 2);
            }
            b'[' => {
                i += 1;
                if i < bytes.len() && bytes[i] == b'^' {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b']' {
                    i += 1;
                }
                while i < bytes.len() && bytes[i] != b']' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = snap(i + 1);
            }
            b'|' => {
                // Only a `|` directly in a conditional body splits it.
                if let Some(Frame::Conditional { at, pipes }) = frames.last_mut() {
                    *pipes += 1;
                    if *pipes > 1 {
                        return Some((
                            "conditional backref with more than two branches".to_string(),
                            i,
                        ));
                    }
                    let _ = at;
                }
                i += 1;
            }
            b')' => {
                frames.pop();
                i += 1;
            }
            b'(' => {
                if bytes.get(i + 1) == Some(&b'?') && bytes.get(i + 2) == Some(&b'(') {
                    // `(?(`ref`)`: validate the reference, then scan the
                    // body as a conditional frame.
                    let name_start = i + 3;
                    let close = pattern[name_start..].find(')').map(|at| name_start + at);
                    let Some(close) = close else {
                        if name_start >= bytes.len() {
                            return Some(("missing group name".to_string(), name_start));
                        }
                        return Some(("missing ), unterminated name".to_string(), name_start));
                    };
                    let name = &pattern[name_start..close];
                    if name.is_empty() {
                        return Some(("missing group name".to_string(), name_start));
                    }
                    if name.bytes().all(|b| b.is_ascii_digit()) {
                        match name.parse::<u64>() {
                            Ok(0) => {
                                return Some(("bad group number".to_string(), name_start));
                            }
                            Ok(value) if value >= MAXGROUPS => {
                                return Some((
                                    format!("invalid group reference {value}"),
                                    name_start,
                                ));
                            }
                            Ok(value) => digit_refs.push((value, name_start)),
                            Err(_) => {
                                return Some((
                                    format!("invalid group reference {name}"),
                                    name_start,
                                ));
                            }
                        }
                    } else {
                        if !is_python_identifier(name) {
                            return Some((
                                format!("bad character in group name '{name}'"),
                                name_start,
                            ));
                        }
                        if !names.contains(&name) {
                            return Some((format!("unknown group name '{name}'"), name_start));
                        }
                    }
                    frames.push(Frame::Conditional { at: i, pipes: 0 });
                    i = close + 1;
                } else if bytes.get(i + 1) == Some(&b'?') && bytes.get(i + 2) == Some(&b'#') {
                    // A comment group closes at its first `)`.
                    match pattern[i + 3..].find(')') {
                        Some(at) => i = i + 3 + at + 1,
                        None => return Some(("missing ), unterminated comment".to_string(), i)),
                    }
                } else if pattern[i..].starts_with("(?P<") {
                    let name_start = i + 4;
                    let Some(end) = pattern[name_start..].find('>').map(|e| name_start + e) else {
                        i += 3;
                        continue;
                    };
                    names.push(&pattern[name_start..end]);
                    groups += 1;
                    frames.push(Frame::Group);
                    i = end + 1;
                } else if pattern[i..].starts_with("(?P=") {
                    // A backreference names an open group; no new group.
                    match pattern[i + 4..].find(')') {
                        Some(at) => i = i + 4 + at + 1,
                        None => i += 4,
                    }
                } else if bytes.get(i + 1) == Some(&b'?') {
                    // `(?:`, `(?i:`, `(?=`, `(?<name>` ...: no capture.
                    frames.push(Frame::Group);
                    i += 2;
                } else {
                    groups += 1;
                    frames.push(Frame::Group);
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    // A conditional body still open at the end of the pattern.
    if let Some(Frame::Conditional { at, .. }) = frames.last() {
        return Some(("missing ), unterminated subpattern".to_string(), *at));
    }
    for (value, at) in digit_refs {
        if value > groups as u64 {
            return Some((format!("invalid group reference {value}"), at));
        }
    }
    None
}

/// Whether `name` is a Python identifier, which is what CPython requires
/// of a conditional's group name (`str.isidentifier`). The ASCII-ish
/// alphabetic/digit split is the same approximation `group_name_check`
/// uses: it accepts exactly the names CPython accepts for the cases that
/// matter and rejects digits, signs, spaces and symbol characters.
fn is_python_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
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
    fn conditional_groups_report_cpython_diagnostics() {
        // The cases CPython's own test_re.py asserts
        // (test_re_groupref_exists_errors / test_re_groupref_overflow),
        // with the positions CPython reports.
        let cases: &[(&str, &str, usize)] = &[
            (r"(?P<a>)(?(0)a|b)", "bad group number", 10),
            (r"()(?(-1)a|b)", "bad character in group name '-1'", 5),
            (r"()(?(+1)a|b)", "bad character in group name '+1'", 5),
            (
                r"()()()()()()()()()()(?(1_0)a|b)",
                "bad character in group name '1_0'",
                23,
            ),
            (r"()(?( 1 )a|b)", "bad character in group name ' 1 '", 5),
            (r"()(?(㊀)a|b)", "bad character in group name '㊀'", 5),
            (r"()(?(¹)a|b)", "bad character in group name '¹'", 5),
            (r"()(?(१)a|b)", "bad character in group name '१'", 5),
            (r"()(?(1", "missing ), unterminated name", 5),
            (r"()(?()", "missing group name", 5),
            (r"()(?(1)a", "missing ), unterminated subpattern", 2),
            (r"()(?(1)a|b", "missing ), unterminated subpattern", 2),
            (
                r"()(?(1)a|b|c",
                "conditional backref with more than two branches",
                10,
            ),
            (
                r"()(?(1)a|b|c)",
                "conditional backref with more than two branches",
                10,
            ),
            (
                r"(?(1)a|b|)",
                "conditional backref with more than two branches",
                8,
            ),
            (r"()(?(2)a)", "invalid group reference 2", 5),
            (r"()(?(foo)a)", "unknown group name 'foo'", 5),
            (
                r"(?P<a>)(?(1073741823))",
                "invalid group reference 1073741823",
                10,
            ),
        ];
        for &(pattern, msg, at) in cases {
            let got = conditional_check(pattern);
            // `at` is CPython's character position; the scan reports bytes.
            let byte_at = pattern
                .char_indices()
                .nth(at)
                .map_or(pattern.len(), |(b, _)| b);
            assert_eq!(
                got,
                Some((msg.to_string(), byte_at)),
                "pattern {pattern:?} (char position {at})"
            );
        }
    }

    #[test]
    fn valid_conditionals_are_left_to_the_engine() {
        // CPython accepts these; the scan must stay silent so the engine
        // reports its own "unsupported" error.
        for pattern in [
            r"()(?(1)a|b)",
            r"(?P<a>)(?(a)b|c)",
            r"()(?(1)(?(1)a|b)|c)",
            r"(?(1)a)(b)",
            r"()(?(01)a)",
            r"()(?(1)[|](?x:a))",
            r"()(?#(?(0))(?(1)a)",
        ] {
            assert_eq!(conditional_check(pattern), None, "pattern {pattern:?}");
        }
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
