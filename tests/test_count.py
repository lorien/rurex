"""`Pattern.count`: a rurex extension (ADR-0020).

The contract is exact: the count equals the number of items the same
pattern's `finditer` yields over the same window, and the number of
values `findall` returns. Both are checked here, and the values are
cross-checked against stdlib `re` where `re` has an answer.
"""

import re as stdre

import pytest

import rurex

PATTERNS = [
    r"\d+",
    r"([a-z]+)( \d+)?",
    r"(?P<x>\w+)=(\d+)",
    "",
    r"\b",
    r"a*",
    r"(a)(b)?(c)?(d)?(e)?",
    r"(?i)AB",
    r"\s",
    r"[0-9]{2,3}",
]

HAYSTACKS = [
    "abc 123 def 456 ghi",
    "12 34 567",
    "",
    "héllo wörld 造字",
    "aaaa",
    "abcabcabc",
]


def test_count_equals_the_iteration_length_for_every_window():
    # The contract: same window, same left-to-right advancement,
    # including CPython's zero-width rule.
    for pat in PATTERNS:
        rx = rurex.compile(pat)
        for hay in HAYSTACKS:
            for pos in range(0, len(hay) + 1, 2):
                for end in (None, *range(0, len(hay) + 1, 3)):
                    if end is None:
                        got = rx.count(hay, pos)
                        want = len(list(rx.finditer(hay, pos)))
                    else:
                        got = rx.count(hay, pos, end)
                        want = len(list(rx.finditer(hay, pos, end)))
                    assert got == want, (pat, hay, pos, end, got, want)


def test_count_matches_re_and_findall_without_a_window():
    for pat in PATTERNS:
        rx, srx = rurex.compile(pat), stdre.compile(pat)
        for hay in HAYSTACKS:
            want = len(srx.findall(hay))
            assert len(rx.findall(hay)) == want, (pat, hay)
            assert rx.count(hay) == want, (pat, hay)


def test_count_equals_len_findall_and_finditer():
    hay = "one 1 two 22 three 333 four"
    for pat in PATTERNS:
        rx = rurex.compile(pat)
        assert rx.count(hay) == len(rx.findall(hay))
        assert rx.count(hay) == len(list(rx.finditer(hay)))
        assert rx.count(hay, 4) == len(list(rx.finditer(hay, 4)))
        assert rx.count(hay, 4, 15) == len(list(rx.finditer(hay, 4, 15)))


def test_count_returns_int_and_accepts_keywords():
    rx = rurex.compile(rb"\d+")
    assert rx.count(b"a1 b22") == 2
    assert isinstance(rx.count(b"a1 b22"), int)
    # `re`'s methods take the haystack and the bounds as keywords.
    assert rx.count(string=b"a1 b22", pos=2) == 1
    assert rx.count(b"a1 b22", endpos=2) == 1
    assert rx.count(string=b"a1 b22", pos=0, endpos=0) == 0


def test_count_window_edges():
    rx = rurex.compile("")
    # A collapsed window still has the empty match at its start, exactly
    # like `re` and like `finditer`.
    for pos, end in ((0, 0), (3, 3), (4, 3), (5, 3), (5, 5)):
        assert rx.count("abc", pos, end) == len(list(rx.finditer("abc", pos, end)))
    assert stdre.compile("").search("abc", 0, 0) is not None


def test_count_bytes_mode_and_bytes_like_haystacks():
    rx = rurex.compile(rb"[a-z]+")
    assert rx.count(b"ab 12 cd") == 2
    assert rx.count(bytearray(b"ab 12 cd")) == 2
    assert rx.count(memoryview(b"ab 12 cd")) == 2
    assert rx.count(bytearray(b"ab 12 cd"), 4) == 1


def test_count_errors_match_re():
    rx = rurex.compile(r"\d+")
    with pytest.raises(TypeError):
        rx.count(123)
    brx = rurex.compile(rb"\d+")
    with pytest.raises(TypeError):
        brx.count("text")
    with pytest.raises(TypeError):
        rx.count()


def test_count_large_haystack_gil_released_path():
    # Above the 4 KB GIL-release threshold: the detached scan must return
    # the same count as the iterating paths.
    hay = ("abc 123 def 456 " * 2000) + "789"
    rx = rurex.compile(r"\d+")
    assert rx.count(hay) == 4001
    assert rx.count(hay) == len(rx.findall(hay))
    assert rx.count(hay) == len(list(rx.finditer(hay)))
    assert len(stdre.compile(r"\d+").findall(hay)) == 4001


def test_count_is_a_pattern_method_only():
    # The surface decision (ADR-0020): the method exists on rurex.Pattern,
    # nothing is added to the module namespace, and `re` has no such name.
    assert not hasattr(stdre.Pattern, "count")
    assert hasattr(rurex.Pattern, "count")
    assert not hasattr(rurex, "count")
