"""Tests for the matching API: Match objects, search/prefixmatch/
fullmatch, findall/finditer, split, sub/subn — differentially compared
against the stdlib re where semantics are shared."""

import copy
import pickle
import re as stdre

import pytest

import rurex


# --- search / prefixmatch / fullmatch / is_match -------------------------

def test_is_match():
    assert rurex.compile(r"\d+").is_match("abc123") is True
    assert rurex.compile(r"\d+").is_match("abcdef") is False


def test_search_match_re():
    for pattern, text in [
        (r"\w+", "hello world"),
        (r"(?P<a>\d+)-(\d+)", "x 12-34 y"),
        (r"a*", "baac"),
        (r"[a-z]+", "ABC def GHI"),
        (r"é+", "caféé"),
    ]:
        ours = rurex.compile(pattern).search(text)
        theirs = stdre.compile(pattern).search(text)
        assert (ours is None) == (theirs is None)
        assert ours.span() == theirs.span()
        assert ours.group() == theirs.group()


def test_prefixmatch_and_match_equivalence():
    for pattern, text in [
        (r"\d+", "123abc"),
        (r"\d+", "abc123"),
        (r"a|ab", "ab"),
        (r"ab|a", "a"),
        (r"^b", "ab"),
    ]:
        ours = rurex.compile(pattern).prefixmatch(text)
        theirs = stdre.compile(pattern).match(text)
        assert (ours is None) == (theirs is None)
        if theirs:
            assert ours.span() == theirs.span()


def test_fullmatch():
    assert rurex.compile(r"\d+").fullmatch("123").span() == (0, 3)
    assert rurex.compile(r"\d+").fullmatch("12a") is None
    # leftmost-first would give 'a'; the anchored variant finds 'aa'.
    assert rurex.compile("a|aa").fullmatch("aa").group() == "aa"
    assert stdre.compile("a|aa").fullmatch("aa").group() == "aa"


def test_pos_endpos_semantics():
    # ^ anchors at the real start (verified CPython behavior), so this
    # finds nothing exactly like re.search(r'^b', 'ab', 1).
    assert rurex.compile(r"^b").search("ab", 1) is None
    assert rurex.compile(r"b").search("ab", 1).span() == (1, 2)
    assert stdre.compile(r"^b").search("ab", 1) is None
    # endpos truncates the window.
    assert rurex.compile(r"\w+").search("abc def", 0, 3).span() == (0, 3)
    assert rurex.compile(r"\w+").search("abc def", 4).span() == (4, 7)
    # endpos = 0 is an empty window, not a missing one (the bounds walk
    # is skipped when neither index is beyond 0).
    assert rurex.compile(r"\w+").search("abc def", 0, 0) is None
    assert stdre.compile(r"\w+").search("abc def", 0, 0) is None
    assert list(rurex.compile(r"\w+").finditer("abc def", 0, 0)) == []
    assert list(stdre.compile(r"\w+").finditer("abc def", 0, 0)) == []
    assert rurex.compile(r"\w+").findall("abc def", 0, 0) == []
    assert rurex.compile(r"").search("abc", 0, 0).span() == (0, 0)
    assert stdre.compile(r"").search("abc", 0, 0).span() == (0, 0)
    # pos beyond the text: empty matches still found at the end (re).
    assert rurex.compile(r"").search("ab", 5).span() == (2, 2)
    # prefixmatch with pos anchors at pos.
    assert rurex.compile(r"b").prefixmatch("ab", 1).span() == (1, 2)
    assert rurex.compile(r"a").prefixmatch("ab", 1) is None


def test_match_pos_endpos_attributes():
    rx = rurex.compile(r"\w+")
    m = rx.search("abc def", 4)
    assert m.pos == 4
    assert m.endpos == 7
    m2 = rx.search("abc def", 0, 3)
    assert (m2.pos, m2.endpos) == (0, 3)


def test_endpos_default_counts_characters():
    # endpos=None reports the character length (not bytes), resolved
    # lazily on first read.
    text = "héllo wörld 造字"
    m = rurex.compile(r"wö").search(text)
    assert m.endpos == len(text)
    assert m.span() == (6, 8)
    # The value is cached per match and stable across reads.
    assert m.endpos == len(text)


# --- Match objects --------------------------------------------------------

def test_match_group_access():
    m = rurex.compile(r"(?P<year>\d{4})-(\d{2})").search("on 2024-10-07")
    assert m.group() == "2024-10"
    assert m.group(0) == "2024-10"
    assert m.group(1) == "2024"
    assert m.group("year") == "2024"
    assert m.group(1, 2) == ("2024", "10")
    assert m[0] == "2024-10"
    assert m["year"] == "2024"
    assert m.groups() == ("2024", "10")
    assert m.groupdict() == {"year": "2024"}
    assert m.start() == 3 and m.end() == 10
    assert m.span() == (3, 10)
    assert m.span(1) == (3, 7)
    assert m.start(2) == 8 and m.end(2) == 10
    assert m.re.pattern == r"(?P<year>\d{4})-(\d{2})"
    assert m.string == "on 2024-10-07"


def test_match_unmatched_groups():
    m = rurex.compile(r"(a)(b)?").search("a")
    assert m.group(2) is None
    assert m.groups() == ("a", None)
    assert m.groups(default="-") == ("a", "-")
    assert m.span(2) == (-1, -1)
    assert m.start(2) == m.end(2) == -1


def test_match_lastindex_lastgroup():
    assert rurex.compile(r"(a)((b)?c)").prefixmatch("abc").lastindex == 2
    assert rurex.compile(r"(?:(a)|(b))").prefixmatch("b").lastindex == 2
    assert rurex.compile(r"(b)(a)?").prefixmatch("b").lastindex == 1
    m = rurex.compile(r"(?P<x>a)(b)").prefixmatch("ab")
    assert m.lastindex == 2
    assert m.lastgroup is None
    m = rurex.compile(r"(a)(?P<x>b)").prefixmatch("ab")
    assert m.lastindex == 2 and m.lastgroup == "x"


def test_match_errors():
    m = rurex.compile(r"(a)").search("a")
    with pytest.raises(IndexError, match="no such group"):
        m.group(5)
    with pytest.raises(IndexError, match="no such group"):
        m["zz"]


def test_match_repr():
    m = rurex.compile(r"a+").search("caaaf")
    assert repr(m) == "<rurex.Match object; span=(1, 4), match='aaa'>"
    assert repr(rurex.compile("").search("")) == "<rurex.Match object; span=(0, 0), match=''>"


def test_unicode_offsets_match_re():
    text = "héllo wörld 造字"
    for pattern in [r"\w+", r"\S+", r"[a-zé]+", r".+?o"]:
        ours = rurex.compile(pattern).search(text)
        theirs = stdre.compile(pattern).search(text)
        assert ours.span() == theirs.span()
        assert ours.group() == theirs.group()


def test_expand():
    m = rurex.compile(r"(?P<x>\w+) (\w+)").search("hello world")
    assert m.expand(r"\2 \1") == "world hello"
    assert m.expand(r"\g<x>!") == "hello!"
    assert m.expand(r"\g<0>!") == "hello world!"
    assert m.expand("no refs\n\t\\") == "no refs\n\t\\"
    assert m.expand(r"\141") == "a"  # octal escape


# --- findall / finditer ---------------------------------------------------

def test_findall_match_re():
    cases = [
        (r"\d+", "a1b22c333"),
        (r"(\d)(\w)?", "1x 2"),
        (r"(a)(b)?", "a ab"),
        (r"a*", "baac"),
        (r"\b", "a b c"),
    ]
    for pattern, text in cases:
        assert rurex.compile(pattern).findall(text) == stdre.compile(pattern).findall(text)


def test_finditer_zero_width_match_re():
    ours = [m.span() for m in rurex.compile(r"a*").finditer("baac")]
    assert ours == [(0, 0), (1, 3), (3, 3), (4, 4)]
    assert ours == [m.span() for m in stdre.compile(r"a*").finditer("baac")]


def test_finditer_lazy():
    it = rurex.compile(r"\d+").finditer("a1b22c333")
    assert next(it).span() == (1, 2)
    assert next(it).span() == (3, 5)
    assert [m.span() for m in it] == [(6, 9)]
    with pytest.raises(StopIteration):
        next(it)


def test_finditer_match_objects_carry_string():
    it = rurex.compile(r"\w+").finditer("one two")
    m = next(it)
    assert m.string == "one two"
    assert m.re.pattern == r"\w+"


def test_finditer_pos_endpos():
    spans = [m.span() for m in rurex.compile(r"\w+").finditer("abc def", 4)]
    assert spans == [(4, 7)]


# --- split ----------------------------------------------------------------

def test_split_match_re():
    cases = [
        (r"[\s,]+", "a b,  c,,d", 0),
        (r"x*", "abc", 0),
        (r"(\d)", "a1b2c", 0),
        (r"(a)(b)?", "a ab", 0),
        (r",", "a,b,c,d", 2),
        (r"\b", "a b c", 0),
    ]
    for pattern, text, maxsplit in cases:
        assert rurex.compile(pattern).split(text, maxsplit) == stdre.compile(pattern).split(text, maxsplit)


# --- sub / subn -----------------------------------------------------------

def test_sub_match_re():
    cases = [
        (r"(\w+) (\w+)", r"\2 \1", "hello world"),
        (r"a*", "-", "baac"),
        (r"\d+", "N", "a1b22c333"),
        (r"x", r"\g<0>\g<0>", "oxo"),
        (r"(a)|(b)", r"[\1]", "ab"),
    ]
    for pattern, repl, text in cases:
        assert rurex.compile(pattern).sub(repl, text) == stdre.compile(pattern).sub(repl, text)


def test_sub_count_and_subn():
    rx = rurex.compile(r"a")
    assert rx.sub("b", "banana", 2) == "bbnbna"
    assert rx.subn("b", "banana") == ("bbnbnb", 3)
    assert rx.subn("b", "banana", 2) == ("bbnbna", 2)


def test_sub_callable():
    rx = rurex.compile(r"\d+")
    assert rx.sub(lambda m: str(len(m[0])), "a123b7c") == "a3b1c"
    assert rx.subn(lambda m: m[0] * 2, "12 34") == ("1212 3434", 2)
    with pytest.raises(TypeError, match="expected str instance"):
        rx.sub(lambda m: 5, "a1")


def test_sub_template_errors_match_re():
    with pytest.raises(rurex.error, match=r"bad escape \\q at position 0"):
        rurex.compile("x").sub(r"\q", "abc")  # eager, even with no match
    with pytest.raises(rurex.error, match="invalid group reference 1 at position 1"):
        rurex.compile("x").sub(r"\1", "x")
    with pytest.raises(IndexError, match="unknown group name"):
        rurex.compile("(a)").sub(r"\g<z>", "a")
    with pytest.raises(rurex.error, match="missing >, unterminated name"):
        rurex.compile("(a)").sub(r"\g<1", "a")


# --- Regex conveniences ---------------------------------------------------

def test_repr_and_flags_display():
    assert repr(rurex.compile("a+")) == "rurex.compile('a+')"
    assert repr(rurex.compile("a+", rurex.I)) == "rurex.compile('a+', rurex.IGNORECASE)"
    assert repr(rurex.compile("a+", rurex.I | rurex.M)) == (
        "rurex.compile('a+', rurex.IGNORECASE|rurex.MULTILINE)"
    )


def test_pickle_roundtrip():
    rx = rurex.compile(r"(?P<x>\d+)", rurex.I | rurex.X)
    clone = pickle.loads(pickle.dumps(rx))
    assert clone.pattern == rx.pattern
    assert clone.flags == rx.flags
    assert clone.search("X42").span() == (1, 3)


def test_copy():
    rx = rurex.compile("a+")
    assert copy.copy(rx).pattern == rx.pattern
    assert copy.deepcopy(rx).pattern == rx.pattern


# --- bytes mode -----------------------------------------------------------

def test_bytes_mode_match_re():
    for pattern, text in [
        (rb"\w+", b"foo bar"),
        (rb"(\w+)-(\w+)", b"foo-bar"),
        (rb"a*", b"baac"),
    ]:
        ours = rurex.compile(pattern).search(text)
        theirs = stdre.compile(pattern).search(text)
        assert ours.span() == theirs.span()
        assert ours.group() == theirs.group()
    assert rurex.compile(rb"\w+").findall(b"caf\xc3\xa9") == [b"caf"]
    # (?u:...) opts into UTF-8-aware matching (engine superset).
    assert rurex.compile(rb"(?u:\w+)").search(b"caf\xc3\xa9").group() == b"caf\xc3\xa9"
    assert rurex.compile(rb"\d+").sub(rb"[\0]", b"a1b") == b"a[\x00]b"
    assert rurex.compile(rb",").split(b"a,b,c") == [b"a", b"b", b"c"]


def test_mode_mismatch_type_errors():
    with pytest.raises(TypeError, match="cannot use a bytes pattern on a string-like object"):
        rurex.compile(rb"a").search("a")
    with pytest.raises(TypeError, match="cannot use a string pattern on a bytes-like object"):
        rurex.compile("a").search(b"a")


def test_match_bytes_repr_and_expand():
    m = rurex.compile(rb"(a)(b)").search(b"ab")
    assert repr(m) == "<rurex.Match object; span=(0, 2), match=b'ab'>"
    assert m.expand(rb"\2\1") == b"ba"
    with pytest.raises(TypeError):
        m.expand(r"\1")


def test_match_not_picklable_like_re():
    import pickle

    m = rurex.compile(r"a").search("a")
    with pytest.raises(TypeError):
        pickle.dumps(m)
    with pytest.raises(TypeError):
        pickle.dumps(stdre.compile(r"a").search("a"))


def test_finditer_is_own_iterator():
    it = rurex.compile(r"\d").finditer("12")
    assert iter(it) is it
