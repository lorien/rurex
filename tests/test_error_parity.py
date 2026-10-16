"""Error-message and position parity with CPython's `re`, locked in
differentially (message text, pos, and lineno/colno where checked).
Every case below previously produced the engine's own wording."""

import re as stdre

import pytest

import rurex

CASES = [
    "[", "[^", "[a", "[a-", r"[\w-b]", r"[a-\w]", r"[b-a]", r"[a-\]]",
    "*", "+", "?", "{1,2}", "*?", "(?:*)", "(?:+)", "(?:(?{1,2}))",
    "\\", r"\x", r"\p{foo}",
    "(", "(a", "(?", "a)", "(a))", "(a)))",
    "a**", "a*{1,2}", "(a)**", "a{1,2}{3,4}", "a+*", "[a]**", "a{2}{3}",
    "a(?i)b", "(?i)a(?m)b", "b(?s)", "a(?x)b",
    "(?P<foo", "(?P<", "(?P<foo>", "(?P<>x)", "(?P<1>x)", "(?P<a.>x)",
    "(?P=<foo>)", "(?P<?foo>)", "(?P<a x>x)", "(?P=", "(?P=)", "(?P=1)",
    "(?P=a)", "(?P=a1)", "(?P=a.)",
    "(?Pxy)", "(?Z)", "(?y)", "(?<(a)", "(?#comment", "a(?Z)",
    "(?P<foo>x)(?P<foo>y)", "(a)(?P<x>b)(?P<x>c)",
    "(?P<a>)(?P=a", "(?P<a>(?P=a))",
]


@pytest.mark.parametrize("pat", CASES)
def test_error_parity(pat):
    try:
        rurex.compile(pat)
        ours = None
    except rurex.PatternError as e:
        ours = (e.msg, e.pos)
    try:
        stdre.compile(pat)
        theirs = None
    except stdre.PatternError as e:
        theirs = (e.msg, e.pos)
    assert ours == theirs, (pat, ours, theirs)


TEMPLATE_CASES = [r"\400", r"\777", r"\123", r"\0", r"\g<1073741823>"]


@pytest.mark.parametrize("repl", TEMPLATE_CASES)
def test_template_error_parity(repl):
    try:
        rurex.sub("x", repl, "x")
        ours = None
    except (rurex.PatternError, IndexError) as e:
        ours = (getattr(e, "msg", str(e)), getattr(e, "pos", None))
    try:
        stdre.sub("x", repl, "x")
        theirs = None
    except (stdre.PatternError, IndexError) as e:
        theirs = (getattr(e, "msg", str(e)), getattr(e, "pos", None))
    assert ours == theirs, (repl, ours, theirs)


def test_still_valid_patterns():
    # Guard against the new rejections over-reaching.
    for pat in [
        "a*+", "a*?", "a+?", "a??", "a{1,2}?", "a{1,2}+",
        "(?i)a", "(?im)a", "(?i:a)b", "a(?i:b)", "(?a:\\w)x",
        "(?x) a", "(?x)#c\n a", "a\\*", "a\\+", "[+*]",
        r"\x41", "(a)*", "(?:a){1,2}", "x(?i:a)y",
    ]:
        rurex.compile(pat)
