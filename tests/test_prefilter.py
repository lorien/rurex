"""Cold non-ASCII str haystacks and the required-first-byte prefilter.

The prefilter is an internal optimization, so these tests only assert
the user-visible contract: skipping a transcode must never change the
answer compared with stdlib `re`, for every query method.
"""

import re as stdre

import rurex

PATTERN = r"@[a-zA-Z][a-zA-Z0-9_]{4,31}"


def cold_arabic():
    # A non-ASCII string with no '@'. This literal is encoded to UTF-8
    # the first time it is used, but each operation below must still
    # produce the same answer whether the cache is cold or warm.
    return "مرحبا بالعالم، كيف حالك؟"


def test_absent_required_literal_matches_re():
    text = cold_arabic()
    ours = rurex.compile(PATTERN)
    ref = stdre.compile(PATTERN)

    assert ours.findall(text) == ref.findall(text) == []
    assert list(ours.finditer(text)) == list(ref.finditer(text)) == []
    assert ours.search(text) is None
    assert ours.match(text) is None
    assert ours.fullmatch(text) is None
    assert ours.is_match(text) is False


def test_present_required_literal_still_matches():
    text = "قناة @telegram_channel هنا"
    ours = rurex.compile(PATTERN)
    ref = stdre.compile(PATTERN)

    assert ours.findall(text) == ref.findall(text) == ["@telegram_channel"]


def test_alignment_false_positive_is_harmless():
    # U+0640 (Arabic tatweel) stores as 0x0640; on little-endian its
    # low byte is 0x40, the ASCII '@' byte the prefilter searches for.
    # The unit-alignment check must reject it. The visible answer must
    # still equal re's, whichever path the prefilter chose.
    text = "\u0640" * 32
    assert rurex.compile(PATTERN).findall(text) == stdre.findall(PATTERN, text) == []


def test_lone_surrogate_without_the_required_byte_is_no_match():
    # The skipped transcode is what re does implicitly: it never
    # encodes, so a lone surrogate is just another code unit. When the
    # required byte is present the transcode still raises; full
    # surrogate parity is tracked in plan.md.
    text = "abc\ud800xyz"
    assert rurex.compile(PATTERN).findall(text) == stdre.findall(PATTERN, text) == []


def test_case_folded_ascii_prefix():
    text = "رابط HTTPS://example.com هنا"
    ours = rurex.compile(r"(?i)https?://")
    ref = stdre.compile(r"(?i)https?://")

    assert ours.findall(text) == ref.findall(text) == ["HTTPS://"]


def test_non_ascii_prefix_disables_the_prefilter():
    # The first character can be non-ASCII, so the prefilter must not
    # skip the transcode. This is a behavior test: the answer is still
    # re's.
    text = "٠١٢٣ test"
    ours = rurex.compile(r"\d+")
    ref = stdre.compile(r"\d+")

    assert ours.findall(text) == ref.findall(text)


def test_groups_and_windows_after_prefilter():
    text = "مقدمة @alpha_1 ثم @beta_22 نهاية"
    ours = rurex.compile(r"@([a-z_]+)(\d+)")
    ref = stdre.compile(r"@([a-z_]+)(\d+)")

    assert ours.findall(text) == ref.findall(text)
    for pos, endpos in [(0, len(text)), (1, len(text)), (0, len(text) - 3),
                        (5, len(text) - 5)]:
        got = ours.findall(text, pos, endpos)
        want = ref.findall(text, pos, endpos)
        assert got == want
