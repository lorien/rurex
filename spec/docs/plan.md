# Open Tasks

Open-task list. Format, statuses, and lifecycle: `spec/skills/task_tracking.md`.
Priorities: higher number = more important; `-1` = deferred.

## Full lone-surrogate parity for str haystacks

Status: new
Priority: 0

rurex transcodes non-ASCII `str` haystacks to UTF-8. CPython's encoder
rejects lone surrogates (U+D800..U+DFFF), so rurex raises
`UnicodeEncodeError` where stdlib `re` matches the in-memory string
normally. The cold-str prefilter returns the normal no-match answer
when the required first byte is absent, but haystacks that reach the
transcoder still raise. Choose and implement full parity: keep
surrogates through matching without corrupting offsets, or add a
UCS-aware path; add differential tests against `re`.

References: ADR-0012, ADR-0013
