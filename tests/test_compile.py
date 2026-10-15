"""Tests for rurex.compile, flags, attributes, and error reporting."""

import re as stdre

import pytest

import rurex


def test_compile_returns_regex():
    assert isinstance(rurex.compile("a+"), rurex.Regex)
    assert isinstance(rurex.Regex("a+"), rurex.Regex)
    assert isinstance(rurex.Regex("a+", rurex.I), rurex.Regex)


def test_attributes():
    pattern = r"(?P<year>\d{4})-(\d{2})"
    rx = rurex.compile(pattern)
    assert rx.pattern == pattern
    assert rx.groups == 2
    assert rx.groupindex == {"year": 1}


def test_bytes_pattern_attributes():
    rx = rurex.compile(rb"\w+")
    assert isinstance(rx.pattern, bytes)
    assert rx.pattern == rb"\w+"
    assert rx.groups == 0
    assert rx.groupindex == {}


def test_flag_constants_match_re():
    assert rurex.I == stdre.I == 2
    assert rurex.M == stdre.M == 8
    assert rurex.S == stdre.S == 16
    assert rurex.U == stdre.U == 32
    assert rurex.X == stdre.X == 64
    assert rurex.A == stdre.A == 256


def test_stdlib_flags_interoperate():
    # re's constants work directly (bit-compatible design).
    assert isinstance(rurex.compile("a", stdre.I | stdre.M), rurex.Regex)
    assert isinstance(rurex.compile("a", stdre.A), rurex.Regex)
    assert isinstance(rurex.compile("a", stdre.U | stdre.X), rurex.Regex)


def test_flags_normalization_like_re():
    # fix_flags: UNICODE forced on for str without ASCII.
    assert rurex.compile("a").flags == stdre.compile("a").flags == stdre.U
    assert rurex.compile("a", rurex.I).flags == stdre.compile("a", stdre.I).flags
    assert rurex.compile("a", rurex.A).flags == stdre.compile("a", stdre.A).flags
    # bytes: flags stored as passed.
    assert rurex.compile(rb"a", rurex.I).flags == stdre.I
    assert rurex.compile(rb"a").flags == 0


def test_unsupported_and_unknown_flags():
    with pytest.raises(ValueError, match="cannot use UNICODE flag with a bytes pattern"):
        rurex.compile(rb"a", rurex.U)
    with pytest.raises(ValueError, match="cannot use LOCALE flag with a str pattern"):
        rurex.compile("a", stdre.L)
    with pytest.raises(ValueError, match="LOCALE is not supported by the rurex engine"):
        rurex.compile(rb"a", stdre.L)
    with pytest.raises(ValueError, match="ASCII and UNICODE flags are incompatible"):
        rurex.compile("a", rurex.A | rurex.U)
    with pytest.raises(ValueError, match="unsupported flag: TEMPLATE"):
        rurex.compile("a", 1)
    with pytest.raises(ValueError, match="unsupported flag: DEBUG"):
        rurex.compile("a", stdre.DEBUG)
    with pytest.raises(ValueError, match="unknown flag bits: 0x1000"):
        rurex.compile("a", 1 << 12)


@pytest.mark.parametrize(
    "pattern",
    [
        "(a",  # unclosed group
        r"a(?=b)",  # lookahead unsupported
        r"(\w)\1",  # backreference unsupported
        "[z-a]",  # invalid class range
        "(?P<x>a)(?P<x>b)",  # duplicate group name
        "(?P<name",  # unclosed capture name
    ],
)
def test_syntax_errors_carry_re_attributes(pattern):
    with pytest.raises(rurex.error) as excinfo:
        rurex.compile(pattern)
    err = excinfo.value
    assert err.pattern == pattern
    assert isinstance(err.msg, str) and err.msg
    assert isinstance(err.pos, int) and err.pos >= 0
    assert err.lineno >= 1 and err.colno >= 1


def test_error_message_format():
    with pytest.raises(rurex.error, match=r"at position \d+$"):
        rurex.compile("(a")
    with pytest.raises(rurex.error, match=r"line 2, column \d+"):
        rurex.compile("a\n[z-a]")


def test_error_position_is_char_index():
    # '(' sits at byte offset 2 but character index 1 in "é(".
    with pytest.raises(rurex.error) as excinfo:
        rurex.compile("é(")
    assert excinfo.value.pos == 1
    assert excinfo.value.lineno == 1
    assert excinfo.value.colno == 2


def test_bytes_error_position_is_byte_index():
    with pytest.raises(rurex.error) as excinfo:
        rurex.compile(rb"\xff(")
    err = excinfo.value
    assert isinstance(err.pos, int) and err.pos >= 0


def test_pattern_type_error():
    with pytest.raises(TypeError, match="str or bytes"):
        rurex.compile(123)


def test_non_utf8_bytes_pattern():
    # Raw non-UTF-8 bytes cannot be pattern text; \xNN escapes work.
    with pytest.raises(rurex.error, match="not valid UTF-8"):
        rurex.compile(b"a\xffb")
    assert isinstance(rurex.compile(rb"a\xffb"), rurex.Regex)


def test_bytes_unicode_superset_optin():
    # Default: ASCII classes (like re). (?u:...) opts into UTF-8-aware
    # matching for a region — an engine capability re lacks.
    assert isinstance(rurex.compile(rb"\w+"), rurex.Regex)
    assert isinstance(rurex.compile(rb"(?u:\w+)"), rurex.Regex)
    assert isinstance(rurex.compile(rb"(?-u)\x7b\xa9(?u:(.*))"), rurex.Regex)


def test_verbose_flag_parses():
    rx = rurex.compile(r"\d+  # the number \n \d+", rurex.X)
    assert rx.groups == 0


# --- documented deviations from re (api.md "Semantic differences") -------

def test_deviation_absolute_end_anchor():
    # re's \Z is the engine's \z; \Z itself is a compile error.
    with pytest.raises(rurex.error):
        rurex.compile(r"a\Z")
    assert rurex.search(r"a\z", "a\n") is None
    assert rurex.search(r"a\z", "a").span() == (0, 1)


def test_deviation_dollar_and_trailing_newline():
    # re's $ matches before a trailing newline; ours matches at the
    # haystack end only, and \n?$ emulates re's behavior.
    assert rurex.search(r"a$", "a\n") is None
    assert stdre.search(r"a$", "a\n").span() == (0, 1)
    assert rurex.search(r"a\n?$", "a\n").span() == (0, 2)


def test_deviation_octal_escapes_rejected():
    with pytest.raises(rurex.error):
        rurex.compile(r"\141")


def test_deviation_locale_rejected_everywhere():
    with pytest.raises(ValueError, match="LOCALE"):
        rurex.compile("a", stdre.L)
    with pytest.raises(ValueError, match="LOCALE"):
        rurex.compile(rb"a", stdre.L)


def test_no_match_alias():
    assert not hasattr(rurex, "match")
    assert not hasattr(rurex.Regex, "match")
    assert hasattr(rurex, "prefixmatch")


def test_swap_greed_inline_flag():
    # (?U) is the engine-native replacement for a SWAP_GREED flag.
    assert rurex.findall(r"(?U)a+", "aaa") == ["a", "a", "a"]
    assert rurex.search(r"(?U)a+", "aaa").span() == (0, 1)
    # Swapped twice: lazy-then-greedy becomes plain greedy again.
    assert rurex.search(r"(?U)a+?", "aaa").span() == (0, 3)
