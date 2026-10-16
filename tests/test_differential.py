"""Systematic differential tests against the stdlib re.

A corpus of patterns and texts is run through every method pair, so
drift in any matching method shows up broadly. Patterns are valid in
both engines (no lookaround/backreferences — the engine rejects
those; see the deviation tests in test_compile.py).
"""

import re as stdre

import rurex

CORPUS = [
    (r"a+", "aaa b aa"),
    (r"(a|b)*c", "ababc zzz"),
    (r"(?P<x>\d{2,4})-(\d)", "x 12-3 y 4567-8"),
    (r"[a-z]+ [A-Z]+", "hello WORLD and bye WORLD"),
    (r"a.*?b", "aXXb aYYb"),
    (r"^foo", "foo\nbar"),
    (r"foo$", "bar\nfoo"),
    (r"\bword\b", "a word here, wording too"),
    (r"\w+@\w+\.com", "mail me at bob@example.com now"),
    (r"([^()]+)", "((nested)) plain"),
    (r"x{2,3}", "yxxXXxxxx"),
    (r".*", "baac"),
    (r"", "abc"),
    (r"a?b?", "aab bac"),
    (r"é+", "caféé crème"),
    (r"\w+造", "汉字制造 and 造字"),
    (r"(?i)AbC", "xabcX ABC abc"),
    (r"(a)(b)?(c)?", "ac ab abc"),
]

BYTES_CORPUS = [
    (rb"\w+", b"foo bar 123"),
    (rb"a+", b"aaa b aa"),
    (rb"(\d)-(\d)", b"x 1-2 y 3-4"),
    (rb"[a-f]+", b"abcXYZdef"),
    (rb".*", b"baac"),
]


def _cmp_search_family(pattern, text):
    ours_rx = rurex.compile(pattern)
    theirs_rx = stdre.compile(pattern)
    for ours, theirs in [
        (ours_rx.search(text), theirs_rx.search(text)),
        (ours_rx.prefixmatch(text), theirs_rx.match(text)),
        (ours_rx.match(text), theirs_rx.match(text)),
        (ours_rx.fullmatch(text), theirs_rx.fullmatch(text)),
    ]:
        assert (ours is None) == (theirs is None), (pattern, text)
        if theirs is not None:
            assert ours.span() == theirs.span(), (pattern, text)
            assert ours.group() == theirs.group(), (pattern, text)


def test_corpus_search_family():
    for pattern, text in CORPUS:
        _cmp_search_family(pattern, text)


def test_corpus_findall_finditer():
    for pattern, text in CORPUS:
        assert rurex.findall(pattern, text) == stdre.findall(pattern, text)
        ours = [m.span() for m in rurex.finditer(pattern, text)]
        theirs = [m.span() for m in stdre.finditer(pattern, text)]
        assert ours == theirs, (pattern, text)


def test_corpus_split():
    for pattern, text in CORPUS:
        for maxsplit in (0, 2):
            assert rurex.split(pattern, text, maxsplit) == stdre.split(
                pattern, text, maxsplit=maxsplit
            ), (pattern, text, maxsplit)


def test_corpus_sub():
    for pattern, text in CORPUS:
        ngroups = stdre.compile(pattern).groups
        template = r"[\1]" if ngroups >= 1 else "X"
        assert rurex.sub(pattern, template, text) == stdre.sub(
            pattern, template, text
        ), (pattern, text)
        assert rurex.sub(pattern, lambda m: str(len(m[0])), text) == stdre.sub(
            pattern, lambda m: str(len(m[0])), text
        ), (pattern, text)


def test_corpus_is_match():
    for pattern, text in CORPUS:
        assert rurex.compile(pattern).is_match(text) == (
            stdre.search(pattern, text) is not None
        )


def test_corpus_bytes():
    for pattern, text in BYTES_CORPUS:
        ours_rx = rurex.compile(pattern)
        theirs_rx = stdre.compile(pattern)
        assert (ours_rx.search(text) is None) == (theirs_rx.search(text) is None)
        if theirs_rx.search(text):
            assert ours_rx.search(text).span() == theirs_rx.search(text).span()
            assert ours_rx.search(text).group() == theirs_rx.search(text).group()
        assert ours_rx.findall(text) == theirs_rx.findall(text)
        assert ours_rx.split(text) == theirs_rx.split(text)
        assert ours_rx.sub(b"X", text) == theirs_rx.sub(b"X", text)
        assert [m.span() for m in ours_rx.finditer(text)] == [
            m.span() for m in theirs_rx.finditer(text)
        ]


FLAG_CASES = [
    (rurex.I, r"foo", "FOO foo"),
    (rurex.M, r"^b", "a\nb\nc"),
    (rurex.M, r"c$", "a\nb\nc"),
    (rurex.S, r"a.b", "a\nb"),
    (rurex.X, r"\d+  # the number\s*\n \s* \d+", "42 # x\n 7"),
    (rurex.A, r"\w+", "café造"),
    (rurex.I | rurex.S, r"A.B", "x\na\nbX"),
]


def test_flags_matrix():
    for flag, pattern, text in FLAG_CASES:
        assert rurex.findall(pattern, text, flag) == stdre.findall(
            pattern, text, flag
        ), (flag, pattern, text)
        ours = rurex.search(pattern, text, flag)
        theirs = stdre.search(pattern, text, flag)
        assert (ours is None) == (theirs is None)
        if theirs:
            assert ours.span() == theirs.span(), (flag, pattern, text)
        # The same flag applied at compile time agrees with the
        # module-level form.
        assert rurex.compile(pattern, flag).findall(text) == rurex.findall(
            pattern, text, flag
        )


WINDOW_PATTERNS = [r"\w+", r"^b", r"a+", r"x?", r"b$"]
WINDOW_TEXTS = ["ab cd", "abc\nb", "baac", ""]


def _std(method, text, pos, endpos):
    # The stdlib methods do not accept endpos=None (ours defaults it).
    if endpos is None:
        return method(text, pos)
    return method(text, pos, endpos)


def test_pos_endpos_matrix():
    for pattern in WINDOW_PATTERNS:
        ours_rx = rurex.compile(pattern)
        theirs_rx = stdre.compile(pattern)
        for text in WINDOW_TEXTS:
            endpos_values = [None, 1, len(text) // 2, len(text)]
            for pos in range(len(text) + 2):
                for endpos in endpos_values:
                    o = ours_rx.search(text, pos, endpos)
                    t = _std(theirs_rx.search, text, pos, endpos)
                    assert (o is None) == (t is None), (pattern, text, pos, endpos)
                    if t is not None:
                        assert o.span() == t.span(), (pattern, text, pos, endpos)
                    o = ours_rx.prefixmatch(text, pos, endpos)
                    t = _std(theirs_rx.match, text, pos, endpos)
                    assert (o is None) == (t is None), (pattern, text, pos, endpos)
                    if t is not None:
                        assert o.span() == t.span(), (pattern, text, pos, endpos)
                    o = ours_rx.match(text, pos, endpos)
                    t = _std(theirs_rx.match, text, pos, endpos)
                    assert (o is None) == (t is None), (pattern, text, pos, endpos)
                    if t is not None:
                        assert o.span() == t.span(), (pattern, text, pos, endpos)
                    o = [m.span() for m in ours_rx.finditer(text, pos, endpos)]
                    t = [m.span() for m in _std(theirs_rx.finditer, text, pos, endpos)]
                    assert o == t, (pattern, text, pos, endpos)
