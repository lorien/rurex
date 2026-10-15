"""Tests for the module-level convenience functions and the pattern
cache — differentially compared against the stdlib re."""

import re as stdre

import pytest

import rurex


def test_search_family_match_re():
    for rurex_fn, re_fn in [
        (rurex.search, stdre.search),
        (rurex.prefixmatch, stdre.match),
        (rurex.fullmatch, stdre.fullmatch),
    ]:
        for pattern, text in [
            (r"\d+", "ab123"),
            (r"a*", "baac"),
            (r"(?P<x>\w+)!", "hey!"),
            (r"é+", "caféé"),
        ]:
            ours = rurex_fn(pattern, text)
            theirs = re_fn(pattern, text)
            assert (ours is None) == (theirs is None)
            if theirs:
                assert ours.span() == theirs.span()
                assert ours.group() == theirs.group()


def test_findall_finditer_match_re():
    for pattern, text in [(r"\d", "a1b2"), (r"a*", "baac"), (r"(\d)(\w)?", "1x 2")]:
        assert rurex.findall(pattern, text) == stdre.findall(pattern, text)
        assert [m.span() for m in rurex.finditer(pattern, text)] == [
            m.span() for m in stdre.finditer(pattern, text)
        ]


def test_split_sub_match_re():
    assert rurex.split(r"[\s,]+", "a b, c", 1) == stdre.split(r"[\s,]+", "a b, c", maxsplit=1)
    assert rurex.split(r"x*", "abc") == stdre.split(r"x*", "abc")
    assert rurex.sub(r"(\w+) (\w+)", r"\2 \1", "hello world") == stdre.sub(
        r"(\w+) (\w+)", r"\2 \1", "hello world"
    )
    assert rurex.subn(r"a", "b", "banana", 2) == stdre.subn(r"a", "b", "banana", count=2)
    assert rurex.sub(r"\d+", lambda m: str(len(m[0])), "a123b7") == stdre.sub(
        r"\d+", lambda m: str(len(m[0])), "a123b7"
    )


def test_flags_argument():
    assert rurex.search("foo", "FOO", rurex.IGNORECASE).span() == (0, 3)
    # stdlib flag constants interoperate.
    assert rurex.search("foo", "FOO", stdre.I).span() == (0, 3)
    assert rurex.findall(r"a.b", "A\nB", stdre.I | stdre.S) == ["A\nB"]
    with pytest.raises(ValueError, match="cannot use UNICODE flag with a bytes pattern"):
        rurex.search(rb"a", b"a", rurex.U)


def test_bytes_patterns():
    assert rurex.findall(rb"\w+", b"x y") == [b"x", b"y"]
    assert rurex.search(rb"\d+", b"ab12").span() == (2, 4)


def test_errors_match_compile():
    with pytest.raises(rurex.error) as excinfo:
        rurex.search("(a", "a")
    err = excinfo.value
    assert err.pattern == "(a" and err.msg == "unclosed group" and err.pos == 0
    with pytest.raises(TypeError, match="pattern must be str or bytes"):
        rurex.search(42, "a")


def test_cache_returns_same_compiled_object():
    a = rurex.search(r"cache\d*", "cache123").re
    b = rurex.search(r"cache\d*", "cache me").re
    assert a is b
    # Different flags compile separately (mirrors re).
    c = rurex.search(r"cache\d*", "cache", rurex.I).re
    assert a is not c


def test_purge_clears_cache():
    a = rurex.search(r"purgeme", "purgeme").re
    b = rurex.search(r"purgeme", "purgeme again").re
    assert a is b
    rurex.purge()
    c = rurex.search(r"purgeme", "purgeme").re
    assert a is not c


def test_cache_is_bounded_fifo():
    # Fill beyond the 512-entry cap; the first pattern must have been
    # evicted (a fresh object appears), while a recent one is retained.
    first = rurex.search(r"first-pattern", "first-pattern").re
    for i in range(600):
        rurex.search(f"filler-{i}-pattern", f"filler-{i}-pattern")
    again = rurex.search(r"first-pattern", "first-pattern").re
    assert first is not again
    recent = rurex.search(r"filler-599-pattern", "filler-599-pattern").re
    assert rurex.search(r"filler-599-pattern", "x filler-599-pattern x").re is recent
