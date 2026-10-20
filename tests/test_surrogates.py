"""Lone surrogates in `str` haystacks must behave like `re`.

CPython permits U+D800..U+DFFF in a `str`; the engine matches UTF-8, so
rurex maps each lone surrogate to a private-use stand-in while matching
and restores the surrogate when it builds result strings. These tests
are differential: every answer is compared with stdlib `re`.
"""

import re as stdlib_re

import pytest

import rurex

HAYSTACKS = [
    "a\ud800b",
    "\ud800",
    "\ud800\udfff",
    "x\ud800",
    "a\ud800b\ud800",
    "\ud800a\udfff",
    "caf\u00e9 \ud800 \u4e2d",
    "a\ud800b\U0001F600c",  # UCS-4 storage: astral character present
    "\U0001F600\ud800",
]

PATTERNS = [
    ".",
    r"\w",
    r"\W",
    r"\S",
    r"[^a]",
    r"a.b",
    r"x.",
    r".y",
    r"$",
    r"b$",
    r"\A.",
]

APIS = ["findall", "search", "match", "fullmatch"]


@pytest.mark.parametrize("text", HAYSTACKS)
@pytest.mark.parametrize("pattern", PATTERNS)
def test_findall_and_search_parity(text, pattern):
    assert rurex.findall(pattern, text) == stdlib_re.findall(pattern, text)

    ours = rurex.search(pattern, text)
    ref = stdlib_re.search(pattern, text)
    assert (ours is None) == (ref is None)
    if ref is not None:
        assert ours.span() == ref.span()
        assert ours.group() == ref.group()


@pytest.mark.parametrize("text", HAYSTACKS)
def test_iteration_parity(text):
    ours = [(m.span(), m.group()) for m in rurex.finditer(".", text)]
    ref = [(m.span(), m.group()) for m in stdlib_re.finditer(".", text)]
    assert ours == ref


@pytest.mark.parametrize("text", HAYSTACKS)
def test_split_sub_parity(text):
    assert rurex.split(r"\W+", text) == stdlib_re.split(r"\W+", text)
    assert rurex.sub(r"\W", "-", text) == stdlib_re.sub(r"\W", "-", text)
    assert rurex.subn(r"\W", "-", text)[1] == stdlib_re.subn(r"\W", "-", text)[1]


@pytest.mark.parametrize("text", HAYSTACKS)
def test_scanner_parity(text):
    ours = [m.group() for m in iter(rurex.compile(r"\w+").scanner(text).search, None)]
    ref = [
        m.group()
        for m in iter(stdlib_re.compile(r"\w+").scanner(text).search, None)
    ]
    assert ours == ref


def test_prefilter_skip_stays_no_match():
    # The required first byte is absent, so the transcode never runs and
    # the answer matches `re` for the same reason it always did.
    assert rurex.findall("@[a-z]+", "abc\ud800xyz") == stdlib_re.findall(
        "@[a-z]+", "abc\ud800xyz"
    ) == []


def test_stand_in_block_collision_raises():
    # A haystack that already contains the stand-in block cannot be
    # matched safely: a mapped surrogate and a real stand-in character
    # would be indistinguishable. This is the one residual corner; it
    # keeps the old error rather than returning a wrong answer.
    with pytest.raises(UnicodeEncodeError):
        rurex.findall(".", "\U000F0000\ud800")


def test_pattern_side_surrogates_still_rejected():
    # Pattern literals and `\uXXXX` escapes carrying a surrogate remain
    # unsupported (tracked separately); a surrogate *haystack* must not
    # change how a normal pattern compiles.
    with pytest.raises((TypeError, rurex.error)):
        rurex.compile("\ud800")
    with pytest.raises(rurex.error):
        rurex.compile(r"\ud800")


def test_stand_in_property_patterns_keep_raising():
    # `\p{Co}` can tell the stand-in block from a surrogate, so the
    # pattern opts out of substitution and the old error remains.
    with pytest.raises(UnicodeEncodeError):
        rurex.findall(r"\p{Co}", "a\ud800")
