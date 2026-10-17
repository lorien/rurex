# EXPECTED ledger — regex-engine-divergences (mode L)

Mode L compares rurex with **live** stdlib `re` on the vendored
constructs (ADR-0011); the harness enforces exhaustiveness in both
directions. Mode E (vs the recorded Rust answers) currently has zero
`discuss` rows and needs no ledger.

| case | bucket |
|---|---|
| anchor-end-string | by-design: re-only syntax (Z) |
| anchor-end-string-only | by-design: engine superset syntax (z) |
| backref-numbered | by-design: re-only syntax (1) |
| backref-recursion-full | by-design: engine superset syntax ((?R)) |
| char-unicode-prop | by-design: engine superset syntax (p) |
| char-unicode-prop-neg | by-design: engine superset syntax (P) |
| class-difference | by-design: engine nested-class syntax re parses as literals |
| class-intersection-icu | by-design: engine class-set syntax (&&) re parses as literals |
| cond-numbered | by-design: re-only syntax ((?() |
| csharp-class-subtraction | by-design: engine nested-class syntax re parses as literals |
| csharp-lookahead | by-design: re-only syntax ((?=) |
| csharp-named-group | by-design: engine superset syntax ((?<) |
| dart-02-lookbehind-positive | by-design: re-only syntax ((?<=) |
| dart-03-lookbehind-negative | by-design: re-only syntax ((?<!) |
| dart-04-unicode-property | by-design: engine superset syntax (p) |
| dart-05-unicode-property-negated | by-design: engine superset syntax (P) |
| dart-08-email-with-unicode | by-design: engine superset syntax (p) |
| edge-csharp-class-subtraction | by-design: engine nested-class syntax re parses as literals |
| edge-csharp-connector-boundary | by-design: word-boundary semantics differ (unicode vs ascii) |
| edge-go-combining-mark | by-design: engine superset syntax (p) |
| edge-go-no-whitespace-prop | by-design: engine superset syntax (p) |
| edge-java-binary-prop | by-design: engine superset syntax (p) |
| edge-java-intersection | by-design: engine class-set syntax (&&) re parses as literals |
| edge-perl-gcb-boundary | by-design: unknown \b{...} extension errors in the engine; re reads \b + literal |
| edge-python-Z-trailing-newline | by-design: re-only syntax (Z) |
| edge-re2-vs-rust-octal | by-design: re-only syntax (octal escape) |
| elixir-named-group | by-design: engine superset syntax ((?<) |
| elixir-pcre-atomic | by-design: re-only syntax ((?>) |
| elixir-pcre-backref | by-design: re-only syntax (1) |
| elixir-pcre-lookahead | by-design: re-only syntax ((?=) |
| elixir-pcre-recursion | by-design: engine superset syntax ((?R)) |
| go-no-backref-error | by-design: re-only syntax (1) |
| go-no-lookaround-error | by-design: re-only syntax ((?=) |
| go-posix-class | by-design: engine nested-class syntax re parses as literals |
| go-unicode-category | by-design: engine superset syntax (p) |
| group-atomic | by-design: re-only syntax ((?>) |
| group-named | by-design: engine superset syntax ((?<) |
| java-binary-prop | by-design: engine superset syntax (p) |
| java-class-intersection | by-design: engine class-set syntax (&&) re parses as literals |
| java-lookahead | by-design: re-only syntax ((?=) |
| java-lookbehind | by-design: re-only syntax ((?<=) |
| java-posix-class | by-design: engine superset syntax (p) |
| java-unicode-category-L | by-design: engine superset syntax (p) |
| js-v-class-difference | by-design: engine superset syntax (p) |
| js-v-class-intersection | by-design: engine class-set syntax (&&) re parses as literals |
| js-v-nested-class | by-design: engine nested-class syntax re parses as literals |
| l2-alternation-in-lookbehind | by-design: re-only syntax ((?<=) |
| l2-anchor-backref | by-design: re-only syntax (1) |
| l2-anchor-lookahead | by-design: re-only syntax ((?=) |
| l2-anchor-lookbehind | by-design: re-only syntax ((?<=) |
| l2-anchor-neg-lookahead | by-design: re-only syntax ((?!) |
| l2-atomic-quantifier | by-design: re-only syntax ((?>) |
| l2-backref-alternation-group | by-design: re-only syntax (1) |
| l2-backref-in-alternation | by-design: re-only syntax (1) |
| l2-backref-in-quantified-group | by-design: re-only syntax (1) |
| l2-boundary-neg-lookahead | by-design: re-only syntax ((?!) |
| l2-capture-optional-backref | by-design: re-only syntax ((?() |
| l2-capture-unicode-named | by-design: engine superset syntax (p) |
| l2-capturing-alternation-backref | by-design: re-only syntax (1) |
| l2-csharp-class-subtraction | by-design: engine nested-class syntax re parses as literals |
| l2-double-lookahead | by-design: re-only syntax ((?=) |
| l2-group-backref | by-design: re-only syntax (1) |
| l2-html-tag-pair | by-design: re-only syntax (1) |
| l2-java-class-intersection | by-design: engine class-set syntax (&&) re parses as literals |
| l2-lookahead-alternation | by-design: re-only syntax ((?=) |
| l2-lookahead-anchor-end | by-design: re-only syntax ((?!) |
| l2-lookahead-backref | by-design: re-only syntax ((?=) |
| l2-lookahead-boundary | by-design: re-only syntax ((?=) |
| l2-lookahead-capture | by-design: re-only syntax ((?=) |
| l2-lookahead-capture-backref | by-design: re-only syntax ((?=) |
| l2-lookahead-class-quantifier | by-design: re-only syntax ((?=) |
| l2-lookahead-lookahead | by-design: re-only syntax ((?=) |
| l2-lookahead-optional-capture | by-design: re-only syntax ((?=) |
| l2-lookahead-quantified-group | by-design: re-only syntax ((?=) |
| l2-lookbehind-anchor-start | by-design: re-only syntax ((?<=) |
| l2-lookbehind-capture | by-design: re-only syntax ((?<=) |
| l2-lookbehind-class | by-design: re-only syntax ((?<=) |
| l2-lookbehind-lookahead | by-design: re-only syntax ((?=) |
| l2-lookbehind-neg-class | by-design: re-only syntax ((?<=) |
| l2-lookbehind-quantifier | by-design: re-only syntax ((?<=) |
| l2-multiple-backrefs | by-design: re-only syntax (1) |
| l2-multiple-named-groups | by-design: engine superset syntax ((?<) |
| l2-named-group-quantifier | by-design: engine superset syntax ((?<) |
| l2-named-group-star | by-design: engine superset syntax ((?<) |
| l2-named-groups-alternation | by-design: engine superset syntax ((?<) |
| l2-neg-lookahead-class | by-design: re-only syntax ((?!) |
| l2-neg-lookahead-neg-lookbehind | by-design: re-only syntax ((?!) |
| l2-neg-lookbehind-capture | by-design: re-only syntax ((?<!) |
| l2-nested-lookaround | by-design: re-only syntax ((?=) |
| l2-nested-named-groups | by-design: engine superset syntax ((?<) |
| l2-non-backtracking-group | by-design: re-only syntax ((?>) |
| l2-possessive-group-class | by-design: re-only syntax ((?>) |
| l2-possessive-in-group | by-design: re-only syntax ((?>) |
| l2-recursion-in-group | by-design: engine superset syntax ((?R)) |
| l2-recursive-nested | by-design: engine superset syntax ((?R)) |
| l2-triple-capture-backref | by-design: re-only syntax (1) |
| l2-unicode-case-insensitive | by-design: engine superset syntax (p) |
| l2-unicode-category-number | by-design: engine superset syntax (p) |
| l2-unicode-digit-boundary | by-design: engine superset syntax (p) |
| l2-unicode-group | by-design: engine superset syntax (p) |
| l2-unicode-group-boundary | by-design: engine superset syntax (p) |
| l2-unicode-prop-anchor | by-design: engine superset syntax (p) |
| l2-unicode-prop-quantifier | by-design: engine superset syntax (p) |
| l2-unicode-script | by-design: engine superset syntax (p) |
| l3-alternation-backref-lazy | by-design: re-only syntax (1) |
| l3-atomic-alternation-quantifier | by-design: re-only syntax ((?>) |
| l3-atomic-backref-anchor | by-design: re-only syntax ((?>) |
| l3-backref-alternation-anchor | by-design: re-only syntax (1) |
| l3-capture-inside-lookahead-reuse | by-design: re-only syntax ((?=) |
| l3-class-intersection-lookahead | by-design: re-only syntax ((?=) |
| l3-edge-atomic-no-backtrack | by-design: re-only syntax ((?>) |
| l3-edge-backref-in-class-literal | by-design: re-only syntax (1) |
| l3-edge-backref-nonparticipating | by-design: re-only syntax (1) |
| l3-edge-empty-capture-backref | by-design: re-only syntax (1) |
| l3-edge-lookahead-at-end | by-design: re-only syntax ((?=) |
| l3-edge-overlapping-capture | by-design: re-only syntax ((?=) |
| l3-edge-quantifier-on-anchor | by-design: engine superset (anchor/boundary repetition) |
| l3-edge-unicode-boundary | by-design: engine superset syntax (p) |
| l3-edge-zero-width-quantified | by-design: re-only syntax ((?=) |
| l3-flag-multiline-lookahead | by-design: re-only syntax ((?=) |
| l3-flavor-csharp-subtraction-anchor | by-design: engine nested-class syntax re parses as literals |
| l3-flavor-perl-unicode-boundary | by-design: unknown \b{...} extension errors in the engine; re reads \b + literal |
| l3-flavor-python-conditional | by-design: re-only syntax ((?() |
| l3-flavor-python-flag-scope | by-design: re-only syntax (1) |
| l3-flavor-python-nested-named | by-design: re-only syntax ((?P=) |
| l3-flavor-rust-unicode-class | by-design: engine superset syntax (p) |
| l3-lookahead-backref-group | by-design: re-only syntax ((?=) |
| l3-lookahead-boundary-quantifier | by-design: re-only syntax ((?=) |
| l3-lookahead-capture-quantified | by-design: re-only syntax ((?=) |
| l3-lookahead-lazy-quant-class | by-design: re-only syntax ((?=) |
| l3-lookahead-lookbehind-capture | by-design: re-only syntax ((?=) |
| l3-lookahead-negated-class-escape | by-design: re-only syntax ((?=) |
| l3-lookbehind-dot-quantifier | by-design: re-only syntax ((?=) |
| l3-lookbehind-negated-class-anchor | by-design: re-only syntax ((?<=) |
| l3-multiple-lookaround | by-design: re-only syntax ((?=) |
| l3-mysql-icu-decimal-class | by-design: engine superset syntax (p) |
| l3-mysql-icu-letter-class | by-design: engine superset syntax (p) |
| l3-mysql-icu-script-greek | by-design: engine superset syntax (p) |
| l3-mysql-named-group-extract | by-design: engine superset syntax ((?<) |
| l3-named-alt-quantifier-anchor | by-design: engine superset syntax ((?<) |
| l3-negated-lookahead-class-quant | by-design: re-only syntax ((?!) |
| l3-negated-lookbehind-class-boundary | by-design: re-only syntax ((?<!) |
| l3-nested-groups-multi-capture | by-design: re-only syntax (1) |
| l3-nested-lookaround-capture | by-design: re-only syntax ((?=) |
| l3-real-balanced-brackets | by-design: engine superset syntax ((?R)) |
| l3-real-camel-to-words | by-design: re-only syntax ((?=) |
| l3-real-duplicate-words | by-design: re-only syntax (1) |
| l3-real-email-lookahead | by-design: re-only syntax ((?=) |
| l3-real-hex-color-named | by-design: engine superset syntax ((?<) |
| l3-real-log-named-groups | by-design: engine superset syntax ((?<) |
| l3-real-password-strength | by-design: re-only syntax ((?=) |
| l3-recursive-nested-parens | by-design: engine superset syntax ((?R)) |
| l3-scoped-flag-lookahead-class | by-design: re-only syntax ((?=) |
| l3-word-boundary-class-backref | by-design: re-only syntax (1) |
| lib-atomic-efficient-hostname-match | by-design: re-only syntax ((?>) |
| lib-atomic-group-efficient-json-like-value-match | by-design: re-only syntax ((?>) |
| lib-atomic-group-efficient-property-parser | by-design: re-only syntax ((?>) |
| lib-gnu-and-word-boundaries-grep-extension | by-design: engine class-set syntax (\<) re parses as literals |
| lib-indices-d-flag-es2022-url-parts-with-match-positions | by-design: engine superset syntax ((?<) |
| lib-lookbehind-extract-amount-after-currency-symbol | by-design: re-only syntax ((?<=) |
| lib-lookbehind-price-in-a-dart-shopping-app | by-design: re-only syntax ((?<=) |
| lib-named-group-name-substitution-ready-key-value | by-design: engine superset syntax ((?<) |
| lib-named-groups-name-dart-port-of-v8-irregexp | by-design: engine superset syntax ((?<) |
| lib-named-groups-parse-a-structured-log-line | by-design: engine superset syntax ((?<) |
| lib-negative-inline-u-ascii-only-word-boundary | by-design: engine superset syntax ((?-u) |
| lib-posix-class-alnum-sanitise-identifier | by-design: engine nested-class syntax re parses as literals |
| lib-python-named-backref-p-name-matched-html-style-tags | by-design: re-only syntax ((?P=) |
| lib-recursive-r-balanced-curly-braces | by-design: engine superset syntax ((?R)) |
| lib-regexp-matches-g-flag-extract-all-tags | by-design: engine nested-class syntax re parses as literals |
| lib-spencer-a-z-anchors-exact-length-hex-color | by-design: re-only syntax (Z) |
| lib-strong-password-8-chars | by-design: re-only syntax ((?=) |
| lib-unicode-property-p-han-detect-cjk-runs | by-design: engine superset syntax (p) |
| lib-z-anchor-validate-identifier-with-no-trailing-newline | by-design: re-only syntax (Z) |
| lookahead-negative | by-design: re-only syntax ((?!) |
| lookahead-positive | by-design: re-only syntax ((?=) |
| lookbehind-negative | by-design: re-only syntax ((?<!) |
| lookbehind-positive | by-design: re-only syntax ((?<=) |
| perl-named-group-angle | by-design: engine superset syntax ((?<) |
| perl-recursion | by-design: engine superset syntax ((?R)) |
| perl-unicode-prop | by-design: engine superset syntax (p) |
| pgsql-03-end-of-string-Z | by-design: re-only syntax (Z) |
| pgsql-11-posix-classes | by-design: engine nested-class syntax re parses as literals |
| php-named-group-angle | by-design: engine superset syntax ((?<) |
| php-possessive-quantifier | by-design: possessive quantifiers (engine: stacked repetition) |
| php-recursion | by-design: engine superset syntax ((?R)) |
| py-conditional-group | by-design: re-only syntax ((?() |
| py-named-backref | by-design: re-only syntax ((?P=) |
| py-strict-end | by-design: re-only syntax (Z) |
| r-named-group | by-design: engine superset syntax ((?<) |
| r-pcre-atomic | by-design: re-only syntax ((?>) |
| r-pcre-backref | by-design: re-only syntax (1) |
| r-pcre-lookahead | by-design: re-only syntax ((?=) |
| r-pcre-lookbehind | by-design: re-only syntax ((?<=) |
| r-pcre-recursion | by-design: engine superset syntax ((?R)) |
| re2-no-backref-error | by-design: re-only syntax (1) |
| re2-no-lookaround-error | by-design: re-only syntax ((?=) |
| re2-posix-class | by-design: engine nested-class syntax re parses as literals |
| ruby-atomic-group | by-design: re-only syntax ((?>) |
| ruby-lookbehind | by-design: re-only syntax ((?<=) |
| ruby-named-class-unicode | by-design: engine superset syntax (p) |
| ruby-named-group | by-design: engine superset syntax ((?<) |
| ruby-posix-class | by-design: engine nested-class syntax re parses as literals |
| rust-no-backref-note | by-design: re-only syntax (1) |
| rust-no-lookaround-note | by-design: re-only syntax ((?=) |
| rust-unicode-category | by-design: engine superset syntax (p) |
| rust-unicode-prop | by-design: engine superset syntax (p) |
| shell-06-posix-classes | by-design: engine nested-class syntax re parses as literals |
| shell-12-gnu-word-boundary | by-design: engine class-set syntax (\<) re parses as literals |
