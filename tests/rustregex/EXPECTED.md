# EXPECTED ledger — rust-regex corpus conformance

Every corpus case that does not pass on a layer has a row here
(bucketed reason), mirroring the ADR-0009 ledger convention for the
vendored CPython suite. The harness enforces exhaustiveness in both
directions: a non-passing case without a row fails the suite, and a
row whose case passes is reported stale and fails the suite.

Layers: **A** = rurex vs corpus expectations (binding conformance),
**B** = rurex vs stdlib `re` on identical inputs (parity).

| Bucket | Meaning |
|---|---|
| unmapped | corpus feature rurex's API cannot express (skipped) |
| by-design: python-parity | rurex deliberately follows CPython `re` where that differs from the engine corpus, or `re` has no equivalent |
| by-design: engine-semantics | rurex keeps the engine's behavior where CPython differs (documented deviation) |
| by-design: rurex superset | pattern uses engine syntax `re` lacks; `re` rejects it or parses it differently |
| corpus-ahead-of-pinned-crate | corpus expects engine behavior added upstream after the Cargo.lock-pinned 1.13.1 |
| follow-up | genuine defect or open question — see `Future-task notes` in the session report |

Regenerate with `python gen_expected.py` (after driver changes or
corpus re-vendoring), then re-review the diff.

| case | layer | bucket | reason |
|---|---|---|---|
| anchored/greedy-earliest | A | unmapped | unmapped option: search-kind=earliest |
| anchored/nongreedy-all | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| anchored/word-boundary-nounicode-01 | A | unmapped | unmapped: unicode=false str mode |
| basic/basic171 | A | unmapped | unmapped: unicode bytes mode |
| basic/basic57 | B | by-design: rurex superset | POSIX class [[:..:]] (re parses as literal chars) |
| basic/basic58 | B | by-design: rurex superset | POSIX class [[:..:]] (re parses as literal chars) |
| basic/basic59 | B | by-design: rurex superset | POSIX class [[:..:]] (re parses as literal chars) |
| basic/basic65 | A | unmapped | unmapped: unicode bytes mode |
| basic/basic66 | A | unmapped | unmapped: unicode bytes mode |
| basic/basic67 | A | unmapped | unmapped: unicode bytes mode |
| basic/basic68 | A | unmapped | unmapped: unicode bytes mode |
| basic/basic86 | A | unmapped | unmapped: unicode bytes mode |
| bytes/case-class-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/case-class-simple-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/case-class-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/case-one-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/case-one-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/dotstar-prefix-unicode | A | unmapped | unmapped: unicode bytes mode |
| bytes/invalid-utf8-anchor-200 | A | follow-up | bytes-mode empty-match position off by one on invalid UTF-8 (upstream fuzz regression) |
| bytes/mixed-dot | A | unmapped | unmapped: unicode bytes mode |
| bytes/mixed-dot | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| bytes/negate-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/negate-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-decimal-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-decimal-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-whitespace-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-whitespace-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-word-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/perl-word-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-ascii-100 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-ascii-200 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-ascii-not | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-unicode | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| bytes/word-boundary-unicode-not | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| crazy/date1 | A | unmapped | unmapped: unicode=false str mode |
| crazy/date2 | A | unmapped | unmapped: unicode=false str mode |
| crazy/date3 | A | unmapped | unmapped: unicode=false str mode |
| crazy/email | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| crazy/email-not | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| crazy/empty10 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| crazy/empty11 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| crazy/neg-class-ascii | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| crazy/neg-class-comma-space | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| crazy/neg-class-letter-space | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| crazy/neg-class-space | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| crazy/neg-class-space-comma | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| crazy/nothing-empty | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| crazy/nothing-something | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| crazy/ranges | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| crazy/ranges-not | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| crlf/basic | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/dot-no-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/end-no-split | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/end-no-split-adjacent | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/end-no-split-adjacent-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/end-no-split-adjacent-lf | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/onepass-wrong-crlf-anchored | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/onepass-wrong-crlf-with-capture | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-end-before-after | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-end-empty | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-end-non-empty | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-no-split | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-no-split-adjacent | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-no-split-adjacent-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| crlf/start-no-split-adjacent-lf | B | by-design: rurex superset | CRLF line mode (?R) |
| earliest/is-ungreedy | A | unmapped | unmapped option: search-kind=earliest |
| earliest/look-end-test | A | unmapped | unmapped option: search-kind=earliest |
| earliest/look-start-test | A | unmapped | unmapped option: search-kind=earliest |
| earliest/no-greedy-100 | A | unmapped | unmapped option: search-kind=earliest |
| earliest/no-greedy-200 | A | unmapped | unmapped option: search-kind=earliest |
| earliest/no-leftmost-first-100 | A | unmapped | unmapped option: search-kind=earliest |
| earliest/no-leftmost-first-200 | A | unmapped | unmapped option: search-kind=earliest |
| empty/100 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| empty/110 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| empty/220 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| empty/230 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| empty/300 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| empty/310 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| empty/600 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| empty/610 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| expensive/backtrack-blow-visited-capacity | B | by-design: rurex superset | Unicode class \p{..} |
| flags/10 | B | by-design: rurex superset | swap-greed flag (?U) |
| flags/11 | A | unmapped | unmapped: unicode=false str mode |
| flags/2 | B | by-design: rurex superset | mid-pattern flag toggles ((?-i)) |
| flags/3 | B | by-design: rurex superset | mid-pattern flag toggles ((?-i)) |
| flags/5 | B | by-design: rurex superset | mid-pattern flag toggles ((?-i)) |
| flags/6 | B | by-design: rurex superset | mid-pattern flag toggles ((?-i)) |
| flags/8 | B | by-design: rurex superset | swap-greed flag (?U) |
| flags/9 | B | by-design: rurex superset | swap-greed flag (?U) |
| iter/empty10 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| iter/empty11 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| iter/nonempty-followedby-empty | A | follow-up | same root cause as the layer-B row |
| iter/nonempty-followedby-empty | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| iter/nonempty-followedby-oneempty | A | follow-up | same root cause as the layer-B row |
| iter/nonempty-followedby-oneempty | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| iter/nonempty-followedby-onemixed | A | follow-up | same root cause as the layer-B row |
| iter/nonempty-followedby-onemixed | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| iter/nonempty-followedby-twomixed | A | follow-up | same root cause as the layer-B row |
| iter/nonempty-followedby-twomixed | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| leftmost-all/alt | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| leftmost-all/dotall | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| leftmost-all/multi | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| leftmost-all/multi | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| line-terminator/carriage | A | unmapped | unmapped option: line-terminator |
| line-terminator/dot-changes-with-line-terminator | A | unmapped | unmapped option: line-terminator |
| line-terminator/non-ascii | A | unmapped | unmapped option: line-terminator |
| line-terminator/non-word-byte | A | unmapped | unmapped option: line-terminator |
| line-terminator/not-line-feed | A | unmapped | unmapped option: line-terminator |
| line-terminator/not-word-boundary-at | A | unmapped | unmapped option: line-terminator |
| line-terminator/nul | A | unmapped | unmapped option: line-terminator |
| line-terminator/word-boundary | A | unmapped | unmapped option: line-terminator |
| line-terminator/word-boundary-at | A | unmapped | unmapped option: line-terminator |
| line-terminator/word-byte | A | unmapped | unmapped option: line-terminator |
| misc/prefix-literal-match-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| multiline/basic1-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic1-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic2-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic2-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic3-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic3-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic4-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic4-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic5-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic5-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic6-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic6-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic7-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic7-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic8-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic8-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/basic9-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/match-line-100-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/match-line-100-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/match-line-200 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| multiline/match-line-200-crlf | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| multiline/match-line-200-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/match-line-200-crlf-cr | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| multiline/match-line-200-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat1-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat1-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat1-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat1-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat10 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat10-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat10-no-multi | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-no-multi-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat10-no-multi-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat10-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat11 | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat11-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat11-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat11-no-multi | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat11-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat11-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat12 | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat12-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat12-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat12-no-multi | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat12-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat12-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat13 | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat13-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat13-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat13-no-multi | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat13-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat13-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat14 | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat14-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat14-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat14-no-multi | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| multiline/repeat14-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat14-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat15-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat15-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat15-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat15-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat15-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat16 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat16-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat16-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat16-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat16-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat16-no-multi | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| multiline/repeat16-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat16-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat17-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat17-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat17-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat17-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat18-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat18-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat18-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat18-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat18-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat2 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| multiline/repeat2-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat2-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat2-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat2-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat2-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat3 | A | follow-up | same root cause as the layer-B row |
| multiline/repeat3 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| multiline/repeat3-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat3-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat3-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat3-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat3-no-multi | A | follow-up | same root cause as the layer-B row |
| multiline/repeat3-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat3-no-multi-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat3-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat3-no-multi-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat3-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat4 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| multiline/repeat4-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat4-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat4-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat4-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat4-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat5 | A | follow-up | same root cause as the layer-B row |
| multiline/repeat5 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| multiline/repeat5-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat5-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat5-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat5-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat5-no-multi | A | follow-up | same root cause as the layer-B row |
| multiline/repeat5-no-multi | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| multiline/repeat5-no-multi-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat5-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat5-no-multi-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat5-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat6-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat6-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat6-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat6-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat7-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat7-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat7-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat7-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat8 | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat8-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat8-no-multi | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-no-multi-crlf | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat8-no-multi-crlf-cr | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| multiline/repeat8-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat9-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat9-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat9-no-multi-crlf | B | by-design: rurex superset | CRLF line mode (?R) |
| multiline/repeat9-no-multi-crlf-cr | B | by-design: rurex superset | CRLF line mode (?R) |
| no-unicode/case1 | A | unmapped | unmapped: unicode=false str mode |
| no-unicode/case2 | A | unmapped | unmapped: unicode=false str mode |
| no-unicode/case4 | A | unmapped | unmapped: unicode=false str mode |
| no-unicode/decimal-ascii | A | unmapped | unmapped: unicode=false str mode |
| no-unicode/dotstar-prefix2 | A | unmapped | unmapped: unicode bytes mode |
| no-unicode/iter1-bytes | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| no-unicode/iter2-bytes | A | unmapped | unmapped: unicode bytes mode |
| no-unicode/mixed | A | unmapped | unmapped: unicode bytes mode |
| no-unicode/mixed | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| no-unicode/negate2 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| no-unicode/space-ascii | A | unmapped | unmapped: unicode=false str mode |
| no-unicode/unanchored-invalid-utf8-match-100 | A | unmapped | unmapped: unicode bytes mode |
| no-unicode/unanchored-invalid-utf8-nomatch | A | unmapped | unmapped: unicode bytes mode |
| no-unicode/word-ascii | A | unmapped | unmapped: unicode=false str mode |
| nullsubexpr/nullsubexpr10 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr20 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr22 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr23 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr24 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr26 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr27 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr28 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr3 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr30 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr31 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr32 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr33 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr34 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr35 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr36 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr37 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr38 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr39 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr40 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr42 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr5 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr6 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr65 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr66 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr69 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr7 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr70 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| nullsubexpr/nullsubexpr9 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| overlapping/alt-all-100 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/alt-leftmost-first-100 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/empty-000 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/empty-alt-000 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/empty-alt-000 | B | follow-up | zero-width iteration parity bug: CPython >=3.7 alternates empty and non-empty matches at the same position; rurex advances by one |
| overlapping/empty-alt-010 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/greedy-dotstar-matches-everything-100 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| overlapping/iter1-bytes | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/iter1-incomplete-utf8 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/iter1-utf8 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-plus-all-100 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-plus-all-110 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-plus-all-200 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-plus-leftmost-first-100 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/repetition-plus-leftmost-first-110 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/repetition-plus-leftmost-first-200 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/repetition-star-all-100 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-star-all-200 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/repetition-star-leftmost-first-100 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/repetition-star-leftmost-first-200 | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/scratch | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| overlapping/scratch | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| overlapping/start-end-rep-all | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| overlapping/start-end-rep-leftmost-first | A | unmapped | unmapped: overlapping search on a single pattern |
| overlapping/ungreedy-dotstar-matches-everything-100 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| regex-lite/case-insensitive-is-ascii-only | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/dot-always-matches-codepoint | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/empty-no-split-codepoint | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/negated-class-always-matches-codepoint | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/perl-class-decimal | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/perl-class-space | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/perl-class-word | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/word-boundary | A | unmapped | unmapped: regex-lite crate semantics |
| regex-lite/word-boundary-negated | A | unmapped | unmapped: regex-lite crate semantics |
| regression/ascii-word-underscore | B | by-design: rurex superset | POSIX class [[:..:]] (re parses as literal chars) |
| regression/empty-flag-expr | A | corpus-ahead-of-pinned-crate | mid-pattern global (?x): accepted upstream after 1.13.1; pinned crate, rurex and re all reject |
| regression/end-not-word-boundary | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/endl-or-word-boundary | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/flags-are-unset | A | corpus-ahead-of-pinned-crate | mid-pattern global (?i): accepted upstream after 1.13.1; pinned crate, rurex and re all reject |
| regression/hir-optimization-out-of-order-class | B | by-design: rurex superset | POSIX class [[:..:]] (re parses as literal chars) |
| regression/i429-0 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-0 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/i429-1 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-10 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-11 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-11 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/i429-12 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-2 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-3 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-3 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/i429-3-utf8 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/i429-4 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-5 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-6 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-7 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-8 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i429-8 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/i429-9 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/i969 | A | unmapped | unmapped option: search-kind=earliest |
| regression/i969 | B | by-design: rurex superset | strict-end anchor \z |
| regression/negated-full-byte-range | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/negated-unicode-word-boundary-dfa-fail | A | by-design: python-parity | zero-width iteration follows CPython; corpus follows the Rust engine |
| regression/prefilter-with-aho-corasick-standard-semantics | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| regression/regex-to-glob | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| regression/reverse-suffix-300 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/reverse-suffix-all-no-early-matches-010 | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| regression/ruff-whitespace-around-keywords | A | unmapped | unmapped: unicode=false str mode |
| regression/stops | A | unmapped | unmapped: unicode bytes mode |
| regression/stops-ascii | A | unmapped | unmapped: unicode bytes mode |
| regression/stops-ascii | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/strange-anchor-non-complete-prefix | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| regression/strange-anchor-non-complete-suffix | B | by-design: rurex superset | anchor repetition (^*/^+ etc.) |
| regression/unicode-case-lower-nocase-flag | B | by-design: rurex superset | Unicode class \p{..} |
| regression/unsorted-binary-search-100 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/unsorted-binary-search-200 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/word-boundary-ascii-capture | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/word-boundary-ascii-no-capture | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/word-boundary-ascii-start-x | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/word-boundary-start-x | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/word-boundary-weird-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/word-boundary-weird-minimal-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| regression/y-or-endl | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| regression/zero-or-end | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| repetition/repetition-expensive90 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive91 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive92 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive93 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive94 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive95 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive96 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| repetition/repetition-expensive97 | B | by-design: engine-semantics | empty-loop capture differs: engine does not re-enter a loop body on an empty iteration (by-design; cf. vendored test_bug_2537) |
| set/basic10 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic10-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/basic10-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic100 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic101 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic102 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic110 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic111 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic120 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic121 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic122 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic130 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic20 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic30 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic40 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic50 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic60 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic60-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/basic60-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic61 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic61-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/basic61-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic70 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic71 | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/basic71 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic80 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic81 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic82 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic90 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/basic91 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-010 | A | unmapped | unmapped: set test with per-search options |
| set/caps-010 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-020 | A | unmapped | unmapped: set test with per-search options |
| set/caps-020 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-030 | A | unmapped | unmapped: set test with per-search options |
| set/caps-030 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-110 | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/caps-110 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-120 | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/caps-120 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/caps-121 | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/caps-121 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty10 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty10-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty10-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty11 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty11-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty11-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty20 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty20-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty20-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty21 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty21-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty21-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty22 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty23 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty30 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty30-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty30-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty31 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty31-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty31-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty40 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/empty40-leftmost-first | A | unmapped | unmapped: corpus set semantics = alternation-priority selection; rurex reports membership |
| set/empty40-leftmost-first | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/nomatch10 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/nomatch20 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/nomatch30 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| set/nomatch40 | B | by-design: no-re-equivalent | re has no RegexSet equivalent |
| substring/ascii-word-end | A | unmapped | unmapped: unicode=false str mode |
| substring/ascii-word-start | A | unmapped | unmapped: unicode=false str mode |
| substring/unicode-word-end | A | by-design: python-parity | endpos window counts as end-of-string for \b in re/rurex; corpus bounds search the full haystack |
| unicode/class-emoji1 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-emoji2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-emoji3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-emoji4 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb1 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb4 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb5 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gcb6 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat1 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat10 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat11 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat12 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat13 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat14 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat15 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat16 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat17 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat18 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat19 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat20 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat21 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat22 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat23 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat24 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat25 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat26 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat27 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat28 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat29 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat30 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat31 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat32 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat33 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat34 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat35 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat36 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat37 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat4 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat5 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat6 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat7 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat8 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-gencat9 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-sentence-break1 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-sentence-break2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-sentence-break3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-sentence-break4 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-sentence-break5 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-word-break1 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-word-break2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-word-break3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-word-break4 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class-word-break5 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class10 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class2 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class3 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class4 | B | by-design: rurex superset | negated Unicode class \P |
| unicode/class5 | B | by-design: rurex superset | negated Unicode class \P |
| unicode/class6 | B | by-design: rurex superset | negated Unicode class \P |
| unicode/class7 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class8 | B | by-design: rurex superset | Unicode class \p{..} |
| unicode/class9 | B | by-design: rurex superset | Unicode class \p{..} |
| utf8/empty-utf8no | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-anchored | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-anchored-bounds | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-anchored-bounds-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-anchored-endbound | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-anchored-endbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-anchored-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-anchored-startbound | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-anchored-startbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-bounds | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-bounds-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8no-startbound | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| utf8/empty-utf8no-startbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-anchored-bounds | A | unmapped | harness: bounds [1, 3] not on char boundaries |
| utf8/empty-utf8yes-anchored-bounds-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-anchored-endbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-anchored-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-anchored-startbound | A | unmapped | harness: bounds [1, 4] not on char boundaries |
| utf8/empty-utf8yes-anchored-startbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-bounds | A | unmapped | harness: bounds [1, 3] not on char boundaries |
| utf8/empty-utf8yes-bounds-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| utf8/empty-utf8yes-startbound | A | unmapped | harness: bounds [1, 4] not on char boundaries |
| utf8/empty-utf8yes-startbound-overlapping | A | unmapped | unmapped: overlapping iteration (match-kind=all) |
| word-boundary-special/word-end-ascii-010 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-020 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-030 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-040 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-050 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-060 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-060-bounds | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-060-bounds | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-070 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-080 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-090 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-ascii-110 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-ascii-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-010 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-020 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-030 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-040 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-040 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-050 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-060 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-060-bounds | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-060-bounds | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-070 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-080 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-090 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-ascii-110 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-end-half-ascii-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-040 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-060-bounds | A | unmapped | harness: bounds [2, 3] not on char boundaries |
| word-boundary-special/word-end-half-unicode-060-bounds | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-half-unicode-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-060-bounds | A | unmapped | harness: bounds [2, 3] not on char boundaries |
| word-boundary-special/word-end-unicode-060-bounds | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-end-unicode-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-010 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-020 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-030 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-040 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-050 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-060 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-060-bounds | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-070 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-080 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-090 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-ascii-110 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-ascii-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-010 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-020 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-030 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-040 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-040 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-050 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-060 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-060-bounds | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-060-noutf8 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary-special/word-start-half-ascii-060-noutf8 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-070 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-080 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-090 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-110 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary-special/word-start-half-ascii-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-carriage | A | unmapped | unmapped: unicode bytes mode |
| word-boundary-special/word-start-half-ascii-carriage | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-customlineterm | A | unmapped | unmapped option: line-terminator |
| word-boundary-special/word-start-half-ascii-customlineterm | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-ascii-linefeed | A | unmapped | unmapped: unicode bytes mode |
| word-boundary-special/word-start-half-ascii-linefeed | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-040 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-060-bounds | A | unmapped | harness: bounds [2, 3] not on char boundaries |
| word-boundary-special/word-start-half-unicode-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-half-unicode-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-010 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-020 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-030 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-050 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-060 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-060-bounds | A | unmapped | harness: bounds [2, 3] not on char boundaries |
| word-boundary-special/word-start-unicode-070 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-080 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-090 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary-special/word-start-unicode-110 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb1 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb10 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb11 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb12 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb13 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb14 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb15 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb16 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb17 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb18 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb19 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb2 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb20 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb21 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb21 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb22 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb23 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb23 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb24 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb25 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb26 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb27 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb28 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb29 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb29 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb3 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb30 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb31 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb31 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb32 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb33 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb33 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb34 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb35 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb36 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb37 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb37 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/nb38 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb39 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb4 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb5 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb6 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb7 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb8 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb9 | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/nb9 | B | follow-up | zero-width/empty-loop iteration semantics differ from CPython (step advancement or loop-capture); needs triage — see report |
| word-boundary/unicode1-only-ascii | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/unicode2-only-ascii | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/unicode3-only-ascii | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/unicode4-only-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/unicode5-not-noutf8 | A | unmapped | unmapped: unicode bytes mode |
| word-boundary/unicode5-not-only-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/unicode5-noutf8 | A | unmapped | unmapped: unicode bytes mode |
| word-boundary/unicode5-only-ascii | A | unmapped | unmapped: utf8=false non-unescape bytes mode |
| word-boundary/wb1 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb10 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb11 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb12 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb13 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb14 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb15 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb16 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb17 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb18 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb19 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb2 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb20 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb21 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb22 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb23 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb24 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb25 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb26 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb27 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb28 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb29 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb3 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb30 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb31 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb32 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb33 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb34 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb35 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb36 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb37 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb38 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb39 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb4 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb40 | B | by-design: rurex superset | engine inline flags (?u, ?-u scoping) |
| word-boundary/wb41 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb42 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb43 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb44 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb5 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb6 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb7 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb8 | A | unmapped | unmapped: unicode=false str mode |
| word-boundary/wb9 | A | unmapped | unmapped: unicode=false str mode |
