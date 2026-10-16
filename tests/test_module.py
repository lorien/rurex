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
        ]
        + ["I", "IGNORECASE", "M", "MULTILINE", "S", "DOTALL"]
        + ["U", "UNICODE", "X", "VERBOSE", "A", "ASCII"]
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
