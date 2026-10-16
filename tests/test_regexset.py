"""Tests for rurex.RegexSet: single-pass multi-pattern membership."""

import pytest

import rurex


def test_membership_and_ordering():
    s = rurex.compile_set([r"\d+", r"[a-z]+", r"foo"])
    assert s.is_match("abc 12") is True
    assert s.matches("abc 12") == [True, True, False]
    assert s.matches("---") == [False, False, False]
    assert s.is_match("---") is False


def test_overlapping_hits():
    s = rurex.compile_set([r"a", r"ab", r"b", r"abc"])
    assert s.matches("xabcy") == [True, True, True, True]


def test_empty_set():
    s = rurex.compile_set([])
    assert s.is_match("anything") is False
    assert s.matches("anything") == []


def test_flags_apply_to_all_patterns():
    s = rurex.compile_set([r"foo", r"bar"], rurex.IGNORECASE)
    assert s.matches("FOO BaR") == [True, True]


def test_generator_input():
    s = rurex.compile_set(p for p in [r"x", r"y"])
    assert s.matches("y") == [False, True]


def test_bytes_mode():
    s = rurex.compile_set([rb"\w+", rb"\d+"])
    assert s.matches(b"foo123") == [True, True]
    assert s.is_match(b" ") is False


def test_flag_errors_mirror_compile():
    with pytest.raises(ValueError, match="cannot use UNICODE flag with a bytes pattern"):
        rurex.compile_set([rb"a"], rurex.U)
    with pytest.raises(ValueError, match="ASCII and UNICODE flags are incompatible"):
        rurex.compile_set(["a"], rurex.A | rurex.U)


def test_syntax_error_attributes_point_at_failing_pattern():
    with pytest.raises(rurex.error) as excinfo:
        rurex.compile_set(["ok", "(a"])
    err = excinfo.value
    assert err.pattern == "(a"
    assert err.msg == "missing ), unterminated subpattern"
    assert err.pos == 0


def test_type_errors():
    with pytest.raises(TypeError, match="patterns must be all str or all bytes"):
        rurex.compile_set(["a", b"b"])
    with pytest.raises(TypeError, match="patterns must be an iterable"):
        rurex.compile_set(42)
    with pytest.raises(TypeError, match="patterns must be str or bytes"):
        rurex.compile_set(["a", 3])
    with pytest.raises(TypeError, match="cannot use a string pattern on a bytes-like object"):
        rurex.compile_set(["a"]).is_match(b"a")
    with pytest.raises(TypeError, match="cannot use a bytes pattern on a string-like object"):
        rurex.compile_set([rb"a"]).matches("a")


def test_non_utf8_bytes_pattern():
    with pytest.raises(rurex.error, match="not valid UTF-8"):
        rurex.compile_set([b"a\xffb"])
    # \xNN escapes produce raw-byte matching under the ASCII default.
    assert rurex.compile_set([rb"a\xffb"]).is_match(b"a\xffb") is True
    assert rurex.compile_set([rb"\xff"]).is_match(b"\xff") is True
