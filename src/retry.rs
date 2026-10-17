//! The must-advance retry engine: CPython >=3.7 re-runs a search at
//! the position of an empty match, requiring a non-empty result, and
//! yields that match before moving on. The public regex API cannot
//! express "best non-empty match at p in preference order", so the
//! retry runs on the vendored PikeVM with its `skip_empty` guard.
//!
//! One engine per compiled pattern, built lazily on the first empty
//! match ever encountered — patterns that never match empty (the
//! overwhelming majority) never pay for it.

use regex_automata::{
    Anchored, Input,
    nfa::thompson,
    util::primitives::{NonMaxUsize, PatternID},
};

use crate::flags;
use crate::vendor::pikevm::{Config as VmConfig, PikeVM as RetryVm};

pub(crate) struct RetryEngine {
    vm: RetryVm,
    /// Capture group count, excluding the implicit group 0.
    groups: usize,
}

impl RetryEngine {
    /// Build the retry engine for `src` (the adjusted engine source)
    /// with the same flag configuration as the main engine, so both
    /// parse and match identically.
    ///
    /// Mirrors what `regex`'s own builders do for str/bytes regexes:
    /// str → syntax `utf8(true)` + NFA `utf8(true)`; bytes → both
    /// false (see `regex`'s `builders.rs`, `build_one_string`/`_bytes`).
    pub(crate) fn build(src: &str, cfg: &flags::Config, bytes_mode: bool) -> Option<RetryEngine> {
        let syntax = regex_automata::util::syntax::Config::new()
            .case_insensitive(cfg.case_insensitive)
            .multi_line(cfg.multi_line)
            .dot_matches_new_line(cfg.dot_matches_new_line)
            .ignore_whitespace(cfg.ignore_whitespace)
            .unicode(cfg.unicode)
            .utf8(!bytes_mode);
        let thompson_cfg = thompson::Config::new().utf8(!bytes_mode);
        let mut builder = RetryVm::builder();
        builder
            .configure(VmConfig::new().skip_empty(true))
            .syntax(syntax)
            .thompson(thompson_cfg);
        let vm = builder.build(src).ok()?;
        let groups = vm
            .get_nfa()
            .group_info()
            .group_len(PatternID::ZERO)
            .checked_sub(1)?;
        Some(RetryEngine { vm, groups })
    }

    /// Best non-empty match starting exactly at byte offset `p`,
    /// within `[p, eb)`, as byte spans of all groups (`None` for a
    /// group that did not participate). `None` when there is no such
    /// match (the caller then advances one unit past `p`).
    pub(crate) fn retry(
        &self,
        hay: &[u8],
        p: usize,
        eb: usize,
    ) -> Option<Vec<Option<(usize, usize)>>> {
        if p >= eb || eb > hay.len() {
            return None;
        }
        let input = Input::new(hay).span(p..eb).anchored(Anchored::Yes);
        let mut cache = self.vm.create_cache();
        let mut slots: Vec<Option<NonMaxUsize>> = vec![None; (self.groups + 1) * 2];
        self.vm.search_slots(&mut cache, &input, &mut slots)?;
        let mut spans = Vec::with_capacity(self.groups + 1);
        for i in 0..=self.groups {
            let span = match (slots[2 * i], slots[2 * i + 1]) {
                (Some(s), Some(e)) => Some((s.get(), e.get())),
                _ => None,
            };
            spans.push(span);
        }
        // skip_empty guarantees a non-empty overall match; treat any
        // degenerate result as "no retry" so the caller falls back.
        let whole = spans[0];
        match whole {
            Some((s, e)) if e > s => Some(spans),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> flags::Config {
        flags::translate(0, false).unwrap()
    }

    #[test]
    fn retry_finds_shadowed_nonempty() {
        // `|b`: preferred match at 1 is empty; retry must find `b`.
        let e = RetryEngine::build("|b", &cfg(), false).unwrap();
        let spans = e.retry(b"abc", 1, 3).unwrap();
        assert_eq!(spans[0], Some((1, 2)));
        assert!(e.retry(b"abc", 0, 3).is_none()); // no non-empty at 0
    }

    #[test]
    fn retry_lazy_expands() {
        // `abc|.*?`: at 3 the lazy preference is empty; retry yields `z`.
        let e = RetryEngine::build("abc|.*?", &cfg(), false).unwrap();
        let spans = e.retry(b"abczzz", 3, 6).unwrap();
        assert_eq!(spans[0], Some((3, 4)));
    }

    #[test]
    fn retry_respects_preference_order() {
        // DFS preference: (ε|a)(ε|bc). On "abc" the (ε,'bc')
        // preference dies on input (needs 'b' at 0), so the retry
        // must yield 'a' — leftmost-first, not shortest or longest.
        let e = RetryEngine::build("(?:(?:|a)(?:|bc))", &cfg(), false).unwrap();
        assert_eq!(e.retry(b"abc", 0, 3).unwrap()[0], Some((0, 1)));
        // On "bc" the (ε,'bc') preference is viable and outranks
        // ('a', …), which dies on input.
        assert_eq!(e.retry(b"bcx", 0, 3).unwrap()[0], Some((0, 2)));
    }

    #[test]
    fn retry_groups_and_flags() {
        let e =
            RetryEngine::build("(?i)(a)(x)?", &flags::translate(2, false).unwrap(), false).unwrap();
        let spans = e.retry(b"xA", 1, 2).unwrap();
        assert_eq!(spans[0], Some((1, 2)));
        assert_eq!(spans[1], Some((1, 2)));
        assert_eq!(spans[2], None);
    }
}
