//! First-byte prefilter for cold non-ASCII `str` haystacks.
//!
//! A `str` must be transcoded to UTF-8 before the engine can scan it,
//! and on a cold, non-ASCII string that transcode is the dominant cost
//! (Telegram corpus: ~38% of the whole `findall` time). Most strings
//! cannot match at all: 77% of the posts contain no `@`, the required
//! first character of the benchmark pattern.
//!
//! `regex-syntax`'s literal extractor already computes, for every
//! pattern, a finite set of prefix literals such that every match
//! starts with one of them. When all of those literals start with an
//! ASCII byte, a match can only exist if that byte occurs somewhere in
//! the UCS storage. If none occurs, the operation's no-match answer is
//! final and the transcode can be skipped.
//!
//! The check is deliberately conservative:
//!
//! - only finite, non-empty literal sequences are used (a huge class
//!   or an unbounded prefix yields an infinite sequence);
//! - only literals whose first byte is ASCII are accepted (a non-ASCII
//!   first character would need a full code-point comparison);
//! - at most three distinct bytes are kept (the `memchr` family);
//! - an empty literal disables the prefilter (the pattern can match
//!   empty, so a match may exist without containing the literal);
//! - the prefilter is only consulted on the cold-transcode path;
//!   ASCII strings, strings with a warm CPython UTF-8 cache, and
//!   legacy objects without a validated layout never see it.
//!
//! The scan reads the PEP 393 UCS buffer through the pointer produced
//! by `utf8::inspect`, under the same validated-layout contract as the
//! transcoder (ADR-0012, ADR-0013). For UCS-2/UCS-4 the raw bytes are
//! searched with `memchr` and each hit is checked against the unit
//! alignment and the zero bytes of the unit; this works on both
//! endiannesses and avoids a scalar per-unit loop.

use regex_syntax::ParserBuilder;
use regex_syntax::hir::Hir;
use regex_syntax::hir::literal::Extractor;

use crate::flags;
use crate::utf8::Kind;

/// A pattern analysis performed once at compile time.
pub(crate) struct Analysis {
    /// The first-byte set, when the pattern has one that is safe to
    /// check against UCS storage.
    pub(crate) first: Option<Prefilter>,
    /// Whether the pattern can match the empty string. Only used to
    /// avoid building the zero-width retry engine for patterns that
    /// cannot possibly need it.
    pub(crate) nullable: bool,
}

/// A small set of ASCII bytes, at least one of which every match must
/// start with.
pub(crate) struct Prefilter {
    bytes: [u8; 3],
    len: u8,
}

impl Prefilter {
    /// The candidate first bytes, sorted by first occurrence.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// True if the UCS buffer may contain a match, judged by the
    /// presence of a candidate first byte.
    ///
    /// # Safety
    ///
    /// `data` must point to `len` code units of `kind` in the running
    /// interpreter's PEP 393 storage, exactly as produced by
    /// `utf8::inspect` under a validated `Layout`.
    pub(crate) unsafe fn may_match_ucs(&self, kind: Kind, len: usize, data: *const u8) -> bool {
        if self.len == 0 || len == 0 {
            return false;
        }
        let width = match kind {
            Kind::Ucs1 => 1,
            Kind::Ucs2 => 2,
            Kind::Ucs4 => 4,
        };
        let bytes = unsafe { std::slice::from_raw_parts(data, len * width) };
        if width == 1 {
            return self
                .bytes()
                .iter()
                .any(|&b| memchr::memchr(b, bytes).is_some());
        }
        self.bytes().iter().any(|&b| contains_wide(bytes, width, b))
    }
}

/// True if `bytes` contains a unit of `width` bytes whose code point is
/// exactly the ASCII byte `needle`.
fn contains_wide(bytes: &[u8], width: usize, needle: u8) -> bool {
    // Native-endian storage: the value occupies the first byte on a
    // little-endian target and the last byte on a big-endian one.
    let needle_off = if cfg!(target_endian = "little") {
        0
    } else {
        width - 1
    };
    let mut start = 0;
    let mut false_hits = 0usize;
    while start < bytes.len() {
        let Some(pos) = memchr::memchr(needle, &bytes[start..]) else {
            return false;
        };
        let i = start + pos;
        if i % width == needle_off {
            let unit_start = i - needle_off;
            if bytes[unit_start..unit_start + width]
                .iter()
                .enumerate()
                .all(|(k, &b)| k == needle_off || b == 0)
            {
                return true;
            }
        }
        false_hits += 1;
        if false_hits >= 64 {
            // The needle is a common byte inside unrelated code units
            // (for example 0x40 inside Arabic text). Stop paying the
            // per-hit check and let the transcode decide; this keeps
            // a pathological input from making the prefilter slower
            // than the work it avoids.
            return true;
        }
        start = i + 1;
    }
    false
}

/// Parse the adjusted pattern under the engine's own configuration and
/// derive the first-byte set and nullability. Any parse failure yields
/// the conservative answer (no prefilter, nullable) — validation has
/// already reported the error to the user, so this is unreachable in
/// practice.
pub(crate) fn analyze(text: &str, cfg: &flags::Config, bytes_mode: bool) -> Analysis {
    let mut parser = ParserBuilder::new();
    parser
        .case_insensitive(cfg.case_insensitive)
        .multi_line(cfg.multi_line)
        .dot_matches_new_line(cfg.dot_matches_new_line)
        .ignore_whitespace(cfg.ignore_whitespace)
        .unicode(cfg.unicode)
        .utf8(!bytes_mode);
    let Ok(hir) = parser.build().parse(text) else {
        return Analysis {
            first: None,
            nullable: true,
        };
    };
    let nullable = hir.properties().minimum_len().is_none_or(|n| n == 0);
    let first = if bytes_mode {
        // Byte patterns are matched without a transcode; the engine's
        // own prefilter already does this work.
        None
    } else {
        from_hir(&hir)
    };
    Analysis { first, nullable }
}

fn from_hir(hir: &Hir) -> Option<Prefilter> {
    let seq = Extractor::new().extract(hir);
    let literals = seq.literals()?;
    let mut bytes = [0u8; 3];
    let mut len = 0usize;
    for lit in literals {
        let first = *lit.as_bytes().first()?;
        // NUL would make the wide scan pathological, and non-ASCII
        // first bytes cannot be compared against UCS units here.
        if first == 0 || first >= 0x80 {
            return None;
        }
        if !bytes[..len].contains(&first) {
            if len == bytes.len() {
                return None;
            }
            bytes[len] = first;
            len += 1;
        }
    }
    (len > 0).then_some(Prefilter {
        bytes,
        len: len as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> flags::Config {
        flags::translate(0, false).unwrap()
    }

    fn first_bytes(pattern: &str) -> Option<Vec<u8>> {
        let a = analyze(pattern, &cfg(), false);
        a.first.map(|p| p.bytes().to_vec())
    }

    #[test]
    fn required_literal_prefix() {
        assert_eq!(
            first_bytes(r"@[a-zA-Z][a-zA-Z0-9_]{4,31}"),
            Some(vec![b'@'])
        );
        assert_eq!(first_bytes("https?://"), Some(vec![b'h']));
        assert_eq!(first_bytes("(?:foo|bar)"), Some(vec![b'f', b'b']));
        assert_eq!(first_bytes("(?i)abc"), Some(vec![b'A', b'a']));
        assert_eq!(first_bytes("a*b"), Some(vec![b'a', b'b']));
    }

    #[test]
    fn unsafe_or_wide_prefixes_disable_the_prefilter() {
        // Nullable: a match may not contain the literal at all.
        assert_eq!(first_bytes("a*"), None);
        assert_eq!(first_bytes("a?"), None);
        // First unit is a class too large to enumerate.
        assert_eq!(first_bytes("[a-zA-Z]"), None);
        // First unit can be non-ASCII.
        assert_eq!(first_bytes(r"\d+"), None);
        // Unicode case folding can match a non-ASCII character.
        assert_eq!(first_bytes("(?i)k"), None);
        // Leading wildcard.
        assert_eq!(first_bytes(".*foo"), None);
    }

    #[test]
    fn nullable_is_reported() {
        for (pat, want) in [
            ("", true),
            ("a*", true),
            ("(?:a|)", true),
            ("a+", false),
            ("abc", false),
            (r"\bfoo\b", false),
        ] {
            assert_eq!(analyze(pat, &cfg(), false).nullable, want, "{pat:?}");
        }
    }

    fn pf(pattern: &str) -> Prefilter {
        analyze(pattern, &cfg(), false).first.expect("prefilter")
    }

    #[test]
    fn ucs1_scan() {
        let p = pf("@x");
        let data = "hello @world".as_bytes();
        assert!(unsafe { p.may_match_ucs(Kind::Ucs1, data.len(), data.as_ptr()) });
        let data = "hello world".as_bytes();
        assert!(!unsafe { p.may_match_ucs(Kind::Ucs1, data.len(), data.as_ptr()) });
    }

    #[test]
    fn ucs2_scan_alignment() {
        let p = pf("@x");
        // 0x0640 is an Arabic letter; on little-endian its bytes are
        // 40 06, whose 0x40 must NOT be mistaken for ASCII '@'.
        let units = [0x0640u16, 0x0627, 0x0644];
        let data = units.as_ptr() as *const u8;
        assert!(!unsafe { p.may_match_ucs(Kind::Ucs2, units.len(), data) });

        let units = [0x0627u16, 0x0040, 0x0628];
        let data = units.as_ptr() as *const u8;
        assert!(unsafe { p.may_match_ucs(Kind::Ucs2, units.len(), data) });
    }

    #[test]
    fn ucs4_scan_alignment() {
        let p = pf("@x");
        // 0x00004100 contains byte 0x41 but is not U+0041.
        let units = [0x0000_4100u32, 0x0000_0627];
        let data = units.as_ptr() as *const u8;
        assert!(!unsafe { p.may_match_ucs(Kind::Ucs4, units.len(), data) });

        let units = [0x0000_0627u32, 0x0000_0040];
        let data = units.as_ptr() as *const u8;
        assert!(unsafe { p.may_match_ucs(Kind::Ucs4, units.len(), data) });
    }

    #[test]
    fn multi_byte_set_finds_any_member() {
        let p = pf("(?:foo|bar)");
        for text in ["xxfooxx", "xxbarxx", "xxbazxx"] {
            let bytes = text.as_bytes();
            let got = unsafe { p.may_match_ucs(Kind::Ucs1, bytes.len(), bytes.as_ptr()) };
            assert_eq!(got, text.starts_with("xxf") || text.starts_with("xxb"));
        }
    }

    #[test]
    fn differential_against_reference_scan() {
        // Every generated UCS-1 string: the prefilter may only say
        // false when the required byte really is absent.
        let p = pf("@x");
        let alphabet = [b'a', b'b', b'@', b'x', 0xE9];
        for code in 0usize..(alphabet.len().pow(4)) {
            let mut n = code;
            let mut s = Vec::new();
            for _ in 0..4 {
                s.push(alphabet[n % alphabet.len()]);
                n /= alphabet.len();
            }
            let has_at = s.contains(&b'@');
            let got = unsafe { p.may_match_ucs(Kind::Ucs1, s.len(), s.as_ptr()) };
            assert!(got || !has_at, "false negative for {s:?}");
            assert!(got == has_at, "false positive for {s:?}");
        }
    }
}
