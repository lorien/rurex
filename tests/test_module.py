"""Scaffold smoke tests for the rurex extension module."""

import re
import types

import pytest

import rurex


def test_version():
    assert isinstance(rurex.__version__, str)
    assert re.fullmatch(r"\d+\.\d+\.\d+", rurex.__version__)


def test_escape():
    assert rurex.escape("a.b*c") == r"a\.b\*c"
    assert rurex.escape("plain") == "plain"
    assert rurex.escape("(a|b)") == r"\(a\|b\)"


def test_error_subclasses_exception():
    assert issubclass(rurex.error, Exception)


def test_patternerror_is_the_class_error_the_alias():
    # Exactly like re (3.13+): one exception class whose canonical
    # name is PatternError; `error` is the module-level legacy alias.
    import re as stdre

    assert rurex.error is rurex.PatternError
    assert rurex.PatternError.__name__ == "PatternError"
    assert rurex.PatternError.__name__ == stdre.PatternError.__name__
    try:
        rurex.compile("(")
    except rurex.error as e:
        assert type(e).__name__ == "PatternError"


def test_public_surface():
    allowed = (
        [
            "__version__",
            "compile",
            "compile_set",
            "escape",
            "purge",
            "search",
            "prefixmatch",
            "match",
            "fullmatch",
            "findall",
            "finditer",
            "split",
            "sub",
            "subn",
            "Pattern",
            "Scanner",
            "RegexSet",
            "Match",
            "FindIter",
            "error",
            "PatternError",
            "RegexFlag",
        ]
        + ["I", "IGNORECASE", "M", "MULTILINE", "S", "DOTALL"]
        + ["U", "UNICODE", "X", "VERBOSE", "A", "ASCII"]
        + ["L", "LOCALE", "DEBUG"]
    )
    # PyO3 maintains __all__ on the extension module; maturin's package
    # shim re-exports the public names through it.
    assert sorted(rurex.__all__) == sorted(allowed)
    public = {name for name in dir(rurex) if not name.startswith("_")}
    # `maturin develop` installs an editable shim package whose
    # `from .rurex import *` leaks the submodule name into dir(); a real
    # wheel install does not. Ignore exactly that artifact.
    if isinstance(getattr(rurex, "rurex", None), types.ModuleType):
        public.discard("rurex")
    assert public == set(allowed) - {"__version__"}


def test_escape_differential():
    import string

    chars = string.punctuation + string.whitespace + "é造ß"
    for ch in chars:
        assert rurex.escape(ch) == re.escape(ch), ch
    for s in ["a.b*c", "", "plain", "(a|b) [x] {1,2}", "a\\b"]:
        assert rurex.escape(s) == re.escape(s)


def test_escape_bytes_differential():
    for b in [b".", b"a\xff", b"\xc3\xa9", b" \t", b"*+?[](){}^$&-#~", b""]:
        assert rurex.escape(b) == re.escape(b), b


def test_escape_type_error():
    with pytest.raises(TypeError, match="str or bytes"):
        rurex.escape(42)


def test_regexflag_enum():
    # Mirrors re (3.11+): an IntFlag with stdlib values, short names
    # as aliases, module-prefixed str/repr, bidirectional interop with
    # plain ints and stdlib re flags.
    import enum
    import re as stdre

    assert isinstance(rurex.I, enum.IntFlag)
    assert rurex.I is rurex.IGNORECASE
    assert rurex.I.value == stdre.I.value == 2
    assert str(rurex.I) == "rurex.IGNORECASE"
    assert str(rurex.I | rurex.M) == "rurex.IGNORECASE|rurex.MULTILINE"
    assert rurex.RegexFlag.__module__ == "rurex"
    assert [f.name for f in rurex.RegexFlag] == [
        "ASCII", "IGNORECASE", "LOCALE", "UNICODE",
        "MULTILINE", "DOTALL", "VERBOSE", "DEBUG",
    ]
    # Deprecated-but-present constants, rejected on use like ours do.
    assert str(rurex.LOCALE) == "rurex.LOCALE"
    # IntFlag members work wherever ints do, both directions.
    assert rurex.compile("a", rurex.I).flags == 34
    assert rurex.compile("a", stdre.IGNORECASE).flags == 34
    assert stdre.compile("a", rurex.I).flags == 34
