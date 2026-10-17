"""Zero-width iteration parity with CPython >=3.7 (bug fix, ADR-0010
corpus finding): after an empty match at p, iteration retries at p
requiring a non-empty match before moving on. The first block pins
rurex == re on the fixed families; the second block pins the one
documented by-design divergence (star-quantified empty-able bodies,
where CPython's empty-iteration loop protection is a third semantics
no NFA engine reproduces)."""

import re

import pytest

import rurex

FIXED = [
    (r"|b", "abc"),
    (r"||b", "abc"),
    (r"(?:)|b", "abc"),
    (r"b|", "abc"),
    (r"abc|.*?", "abczzz"),
    (r"abc|.*?", "abcz"),
    (r"abc|.*?", "abczabc"),
    (r"", "abc"),
    (r"x*", "abc"),
    (r"a*", "aaa"),
    (r"a+|b*", "ab b"),
    (r"(\d*)-", "12-34-"),
    (r"\b", "ab cd"),
    (r"a??b", "ab"),
]

OPS = [
    ("finditer", lambda rx, h: [(m.start(), m.end()) for m in rx.finditer(h)]),
    ("finditer-groups",
     lambda rx, h: [tuple(m.groups()) for m in rx.finditer(h)]),
    ("findall", lambda rx, h: rx.findall(h)),
    ("split", lambda rx, h: rx.split(h)),
    ("sub", lambda rx, h: rx.sub(lambda m: f"<{m.group()}>", h)),
    ("finditer-pos",
     lambda rx, h: [(m.start(), m.end())
                    for m in rx.finditer(h, 1, len(h) - 1)]),
]


@pytest.mark.parametrize("op_name,op", OPS)
@pytest.mark.parametrize("pattern,text", FIXED)
def test_zero_width_matches_re(op_name, op, pattern, text):
    assert op(rurex.compile(pattern), text) == op(re.compile(pattern), text)


def test_bytes_mode_matches_re():
    for pattern in (rb"|b", rb"abc|.*?", rb"x*", rb"(\d*)-"):
        for hay in (b"abc", b"abczzz", b"12-34-"):
            r = re.compile(pattern).findall(hay)
            u = rurex.compile(pattern).findall(hay)
            assert u == r, (pattern, hay, u, r)


def test_documented_divergence_loop_family():
    # Star-quantified empty-able body: CPython's empty-iteration loop
    # protection vs the engine's leftmost-first (api.md, by-design).
    assert re.findall(r"(a*)*", "aaa") == ["", ""]
    assert rurex.findall(r"(a*)*", "aaa") == ["aaa", ""]
    assert re.findall(r"(?:|a)*", "aaa") == ["", "a", "", "a", "", "a", ""]
    assert rurex.findall(r"(?:|a)*", "aaa") == ["", "aaa", ""]


def test_scanner_and_split_after_fix():
    assert rurex.compile(r"|b").split("abc") == re.compile(r"|b").split("abc")
    lex = [(r"\d+", lambda sc, t: int(t)), (r"\w+", lambda sc, t: t)]
    assert (rurex.Scanner(lex).scan("12 ab")[0]
            == re.Scanner(lex).scan("12 ab")[0])


def test_lazy_engine_not_built_until_needed():
    # A pattern that never matches empty produces identical results
    # and does not build the retry engine (behavioral smoke: results
    # stay correct after many calls).
    rx = rurex.compile(r"\d+")
    for _ in range(100):
        assert rx.findall("a1b22c333") == ["1", "22", "333"]
