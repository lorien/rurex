"""Error-message and position parity: rurex targets the Python
3.13+ `re` wording on EVERY interpreter version (that is the
documented contract; see api.md). The EXPECTED tables below are
recorded from Python 3.13 and validated live against 3.13/3.14/3.15
in CI. On 3.13+ the local stdlib is additionally compared live, as a
cross-check of the tables themselves.

The historic `re.error` name is used for stdlib exceptions: it exists
on every supported Python (pre-3.13 has no `re.PatternError`)."""

import re as stdre
import sys

import pytest

import rurex

EXPECTED = {
    '[': ('unterminated character set', 0),
    '[^': ('unterminated character set', 0),
    '[a': ('unterminated character set', 0),
    '[a-': ('unterminated character set', 0),
    '[\\w-b]': ('bad character range \\w-b', 1),
    '[a-\\w]': ('bad character range a-\\w', 1),
    '[b-a]': ('bad character range b-a', 1),
    '[a-\\]]': ('bad character range a-\\]', 1),
    '*': ('nothing to repeat', 0),
    '+': ('nothing to repeat', 0),
    '?': ('nothing to repeat', 0),
    '{1,2}': ('nothing to repeat', 0),
    '*?': ('nothing to repeat', 0),
    '(?:*)': ('nothing to repeat', 3),
    '(?:+)': ('nothing to repeat', 3),
    '(?:(?{1,2}))': ('unknown extension ?{', 4),
    '\\': ('bad escape (end of pattern)', 0),
    '\\x': ('incomplete escape \\x', 0),
    '\\p{foo}': ('bad escape \\p', 0),
    '(': ('missing ), unterminated subpattern', 0),
    '(a': ('missing ), unterminated subpattern', 0),
    '(?': ('unexpected end of pattern', 2),
    'a)': ('unbalanced parenthesis', 1),
    '(a))': ('unbalanced parenthesis', 3),
    '(a)))': ('unbalanced parenthesis', 3),
    'a**': ('multiple repeat', 2),
    'a*{1,2}': ('multiple repeat', 2),
    '(a)**': ('multiple repeat', 4),
    'a{1,2}{3,4}': ('multiple repeat', 6),
    'a+*': ('multiple repeat', 2),
    '[a]**': ('multiple repeat', 4),
    'a{2}{3}': ('multiple repeat', 4),
    'a(?i)b': ('global flags not at the start of the expression', 1),
    '(?i)a(?m)b': ('global flags not at the start of the expression', 5),
    'b(?s)': ('global flags not at the start of the expression', 1),
    'a(?x)b': ('global flags not at the start of the expression', 1),
    '(?P<foo': ('missing >, unterminated name', 4),
    '(?P<': ('missing group name', 4),
    '(?P<foo>': ('missing ), unterminated subpattern', 0),
    '(?P<>x)': ('missing group name', 4),
    '(?P<1>x)': ("bad character in group name '1'", 4),
    '(?P<a.>x)': ("bad character in group name 'a.'", 4),
    '(?P=<foo>)': ("bad character in group name '<foo>'", 4),
    '(?P<?foo>)': ("bad character in group name '?foo'", 4),
    '(?P<a x>x)': ("bad character in group name 'a x'", 4),
    '(?P=': ('missing group name', 4),
    '(?P=)': ('missing group name', 4),
    '(?P=1)': ("bad character in group name '1'", 4),
    '(?P=a)': ("unknown group name 'a'", 4),
    '(?P=a1)': ("unknown group name 'a1'", 4),
    '(?P=a.)': ("bad character in group name 'a.'", 4),
    '(?Pxy)': ('unknown extension ?Px', 1),
    '(?Z)': ('unknown extension ?Z', 1),
    '(?y)': ('unknown extension ?y', 1),
    '(?<(a)': ('unknown extension ?<(', 1),
    '(?#comment': ('missing ), unterminated comment', 0),
    'a(?Z)': ('unknown extension ?Z', 2),
    '(?P<foo>x)(?P<foo>y)': ("redefinition of group name 'foo' as group 2; was group 1", 14),
    '(a)(?P<x>b)(?P<x>c)': ("redefinition of group name 'x' as group 3; was group 2", 15),
    '(?P<a>)(?P=a': ('missing ), unterminated name', 11),
    '(?P<a>(?P=a))': ('cannot refer to an open group', 10),
}

TEMPLATE_EXPECTED = {
    '\\400': ('octal escape value \\400 outside of range 0-0o377', 0),
    '\\777': ('octal escape value \\777 outside of range 0-0o377', 0),
    '\\123': None,
    '\\0': None,
    '\\g<1073741823>': ('invalid group reference 1073741823', 3),
}

def _live(pattern):
    try:
        stdre.compile(pattern)
        return None
    except stdre.error as e:
        return (e.msg, e.pos)


@pytest.mark.parametrize("pat", list(EXPECTED))
def test_error_parity(pat):
    try:
        rurex.compile(pat)
        ours = None
    except rurex.PatternError as e:
        ours = (e.msg, e.pos)
    assert ours == EXPECTED[pat], (pat, ours, EXPECTED[pat])
    if sys.version_info >= (3, 13):
        assert _live(pat) == EXPECTED[pat], (
            pat,
            "recorded table drifted from this interpreter's re",
            _live(pat),
            EXPECTED[pat],
        )


def _live_sub(repl):
    try:
        stdre.sub("x", repl, "x")
        return None
    except (stdre.error, IndexError) as e:
        return (getattr(e, "msg", str(e)), getattr(e, "pos", None))


@pytest.mark.parametrize("repl", list(TEMPLATE_EXPECTED))
def test_template_error_parity(repl):
    try:
        rurex.sub("x", repl, "x")
        ours = None
    except (rurex.PatternError, IndexError) as e:
        ours = (getattr(e, "msg", str(e)), getattr(e, "pos", None))
    assert ours == TEMPLATE_EXPECTED[repl], (repl, ours, TEMPLATE_EXPECTED[repl])
    if sys.version_info >= (3, 13):
        assert _live_sub(repl) == TEMPLATE_EXPECTED[repl], (
            repl,
            "recorded table drifted from this interpreter's re",
            _live_sub(repl),
            TEMPLATE_EXPECTED[repl],
        )


def test_still_valid_patterns():
    # Guard against the rejections over-reaching.
    for pat in [
        "a*+", "a*?", "a+?", "a??", "a{1,2}?", "a{1,2}+",
        "(?i)a", "(?im)a", "(?i:a)b", "a(?i:b)", "(?a:\\w)x",
        "(?x) a", "(?x)#c\n a", "a\\*", "a\\+", "[+*]",
        r"\x41", "(a)*", "(?:a){1,2}", "x(?i:a)y",
    ]:
        rurex.compile(pat)
