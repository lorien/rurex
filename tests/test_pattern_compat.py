"""Compatibility tests for `rurex.Pattern` against `re.Pattern`
(ADR-0008): value equality, weakrefs, typing subscription, read-only
`groupindex`, and `scanner` semantics."""

import re as stdre
import types
import weakref

import pytest

import rurex


def test_pattern_value_equality():
    # Like re.Pattern: equal iff same pattern text and same flags;
    # str and bytes patterns never compare equal.
    assert rurex.compile("a") == rurex.compile("a")
    assert rurex.compile("a", rurex.I) == rurex.compile("a", rurex.I)
    assert rurex.compile("a") != rurex.compile("a", rurex.I)
    assert rurex.compile("a") != rurex.compile("b")
    assert rurex.compile("a") != rurex.compile(b"a")
    assert rurex.compile("a") != "a"
    assert rurex.compile("a") != 42
    # Mirrors stdlib behavior exactly.
    assert (stdre.compile("a") == stdre.compile("a")) == (
        rurex.compile("a") == rurex.compile("a")
    )
    assert (stdre.compile("a") == stdre.compile("a", stdre.I)) == (
        rurex.compile("a") == rurex.compile("a", rurex.I)
    )


def test_pattern_hash_matches_equality():
    a, b = rurex.compile(r"(?P<x>a)+"), rurex.compile(r"(?P<x>a)+")
    assert a == b
    assert hash(a) == hash(b)
    # Usable as dict keys, looked up by equal pattern like re.Pattern.
    d = {rurex.compile("x"): 1}
    assert d[rurex.compile("x")] == 1
    # Equal-but-distinct stdlib patterns behave the same way.
    dd = {stdre.compile("x"): 1}
    assert dd[stdre.compile("x")] == 1


def test_pattern_weakref():
    rx = rurex.compile("a")
    ref = weakref.ref(rx)
    assert ref() is rx
    # re.Match is not weakref-able either — keep parity.
    with pytest.raises(TypeError):
        weakref.ref(rx.search("a"))


def test_groupindex_is_read_only_mapping():
    rx = rurex.compile(r"(?P<year>\d{4})-(\d{2})")
    rp = stdre.compile(r"(?P<year>\d{4})-(\d{2})")
    assert rx.groupindex == rp.groupindex
    assert isinstance(rx.groupindex, type(rp.groupindex))  # mappingproxy
    assert not isinstance(rx.groupindex, dict)
    with pytest.raises(TypeError):
        rx.groupindex["year"] = 5


def test_typing_subscription():
    assert rurex.Pattern[str] is not None
    assert rurex.Match[str] is not None
    assert "Pattern" in repr(rurex.Pattern[str])
    assert "Match" in repr(rurex.Match[str])


def test_class_name_is_pattern():
    rx = rurex.compile("a")
    assert type(rx).__name__ == "Pattern"
    assert type(rx).__qualname__ == "Pattern"
    assert "Pattern" in repr(type(rx))


def _scanner_calls(p, text, calls):
    """Run a match/search call sequence against both engines."""
    ours = p.scanner(text)
    theirs = stdre.compile(p.pattern).scanner(text)
    for method_name in calls:
        o = getattr(ours, method_name)()
        t = getattr(theirs, method_name)()
        assert (o is None) == (t is None), (method_name, text, calls)
        if t is not None:
            assert o.span() == t.span(), (method_name, text, calls)
            assert o.group() == t.group()
    return ours


def test_scanner_search_sequence():
    rx = rurex.compile(r"\w+")
    sc = _scanner_calls(rx, "ab cd ef", ["search", "search", "search", "search"])
    assert sc.search() is None  # exhausted stays exhausted


def test_scanner_match_sequence():
    rx = rurex.compile(r"\w+")
    _scanner_calls(rx, "ab cd", ["match", "match", "match"])


def test_scanner_failed_match_consumes():
    # SRE_Scanner parity: a failed match() consumes the match it
    # rejected — after the miss at the gap, search() finds nothing.
    # (Verified against stdlib; surprising but deliberate.)
    rx = rurex.compile(r"\w+")
    sc = rx.scanner("ab cd")
    assert sc.match().span() == (0, 2)
    assert sc.match() is None
    assert sc.search() is None  # "cd" was consumed by the failed match
    std = stdre.compile(r"\w+").scanner("ab cd")
    assert std.match().span() == (0, 2)
    assert std.match() is None
    assert std.search() is None


def test_scanner_pattern_attribute():
    rx = rurex.compile(r"(?P<x>a)b")
    sc = rx.scanner("ab")
    assert sc.pattern is rx


def test_scanner_empty_matches_and_bytes():
    rx = rurex.compile("x*")
    _scanner_calls(rx, "abc", ["search", "search", "search", "search"])
    b = rurex.compile(rb"\w+")
    ours, theirs = b.scanner(b"ab cd"), stdre.compile(rb"\w+").scanner(b"ab cd")
    o, t = ours.search(), theirs.search()
    assert o.span() == t.span() == (0, 2)
    assert o.group() == t.group() == b"ab"
    assert ours.search().span() == theirs.search().span() == (3, 5)
