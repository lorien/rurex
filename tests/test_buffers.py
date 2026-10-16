"""Bytes-like haystacks and repls: bytearray, memoryview, array,
bytes subclasses — differentially compared against stdlib `re`
(CPython accepts any buffer where a bytes pattern scans)."""

import array
import re as stdre

import pytest

import rurex


class B(bytes):
    pass


HAYSTACKS = [
    b"abc abc",
    bytearray(b"abc abc"),
    memoryview(b"abc abc"),
    array.array("b", b"abc abc"),
    B(b"abc abc"),
]


@pytest.mark.parametrize("h", HAYSTACKS, ids=lambda h: type(h).__name__)
def test_search_family(h):
    rx, rre = rurex.compile(b"b"), stdre.compile(b"b")
    m, t = rx.search(h), rre.search(h)
    assert m.span() == t.span() == (1, 2)
    assert m.group() == t.group() == b"b"
    assert m.string is h  # the original object, like re
    prx, prre = rurex.compile(b"ab"), stdre.compile(b"ab")
    assert prx.prefixmatch(h).span() == prre.match(h).span() == (0, 2)
    assert rurex.compile(b"abc").fullmatch(h) is None


@pytest.mark.parametrize("h", HAYSTACKS, ids=lambda h: type(h).__name__)
def test_findall_split_sub(h):
    rx, rre = rurex.compile(rb"a| "), stdre.compile(rb"a| ")
    assert rurex.findall(rb"a", h) == stdre.findall(rb"a", h) == [b"a", b"a"]
    assert rurex.split(b" ", h) == stdre.split(b" ", h) == [b"abc", b"abc"]
    got = rurex.sub(b"a", b"X", h)
    assert got == stdre.sub(b"a", b"X", bytes(h)) == b"Xbc Xbc"


def test_bytes_like_repl():
    h = bytearray(b"xyz")
    # bytearray repl on bytes pattern, and bytes repl on bytearray text.
    assert rurex.sub(b"y", bytearray(b"a"), h) == b"xaz"
    assert stdre.sub(b"y", bytearray(b"a"), h) == b"xaz"
    assert rurex.subn(b"y", memoryview(b"a"), b"xy") == (b"xa", 1)


def test_callable_returning_bytes_like():
    out = rurex.sub(b"y", lambda m: bytearray(b"!"), b"xy")
    assert out == b"x!"


def test_group_on_array_haystack_is_bytes():
    a = array.array("h", [0x6261, 0x6463])  # 'ab', 'cd' (little-endian)
    m = rurex.search(b"ab", a)
    t = stdre.search(b"ab", a)
    assert m.span() == t.span()
    assert m.group() == t.group() == b"ab"
    assert m.string is a


def test_iteration_snapshots_bytes_like():
    # A bytes-like input is copied once: mutation during iteration is
    # invisible (re raises BufferError instead — documented deviation).
    ba = bytearray(b"ab cd")
    it = rurex.compile(rb"\w+").finditer(ba)
    first = next(it)
    assert first.string is ba
    ba.extend(b"  ef")
    assert [m.span() for m in it] == [(3, 5)]
    sc = rurex.compile(rb"\w+").scanner(bytearray(b"ab cd"))
    assert [sc.search().group() for _ in range(2)] == [b"ab", b"cd"]


def test_error_parity_unchanged():
    with pytest.raises(TypeError, match="cannot use a bytes pattern"):
        rurex.search(b"a", "s")
    with pytest.raises(TypeError, match="got 'int'"):
        rurex.search(b"a", 5)
    with pytest.raises(TypeError, match="repl must be bytes or callable"):
        rurex.sub(b"a", "x", b"a")
