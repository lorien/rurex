"""Register `python/rurex` and `python/rurex-bytes` in a rebar checkout.

Run from the root of a BurntSushi/rebar clone after copying
`benchmarks/rebar/main.py` to `engines/python/rurex/main.py` and pasting
the `[[engine]]` blocks from `benchmarks/rebar/README.md` into
`benchmarks/engines.toml`:

    python3 benchmarks/rebar/add_engines.py /path/to/rebar

`python/rurex` mirrors `python/re`: the haystack is `str` when a
benchmark sets `unicode = true` and `bytes` otherwise. So it is listed
on every benchmark that lists `python/re`.

`python/rurex-bytes` hands the binding the UTF-8 bytes even when the
benchmark asks for Unicode mode, which is only sound where the two
modes agree on the match count: an ASCII haystack (Unicode classes and
`\b` see the same characters) on a Unicode-mode benchmark. rebar's
benchmark set has exactly one such benchmark, `dictionary/search/english-15`.

The script is idempotent: engines already present are left alone.
"""

import pathlib
import re
import sys

BYTES_BENCH = ("dictionary/search.toml", "english-15")


def patch_file(path, text):
    lines = text.splitlines(keepends=True)
    additions = 0
    i = 0
    while i < len(lines):
        if lines[i].strip() != "[[bench]]":
            i += 1
            continue
        j = i + 1
        while j < len(lines) and lines[j].strip() != "[[bench]]":
            j += 1
        block = lines[i:j]
        body = "".join(block)
        name = re.search(r'^name\s*=\s*"([^"]+)"', body, re.M)
        name = name.group(1) if name else ""
        lists_re = "'python/re'" in body or '"python/re"' in body
        wants_bytes = BYTES_BENCH[1] == name and BYTES_BENCH[0] in str(path)

        if lists_re or wants_bytes:
            start = next(
                (k for k, line in enumerate(block)
                 if line.strip().startswith("engines = [")),
                None,
            )
            if start is not None:
                end = start
                while end < len(block) and "]" not in block[end]:
                    end += 1
                indent = re.match(r"\s*", block[start]).group(0)
                insert = []
                if lists_re and "python/rurex'" not in body and 'python/rurex"' not in body:
                    insert.append(f"{indent}  'python/rurex',\n")
                if wants_bytes and "python/rurex-bytes" not in body:
                    insert.append(f"{indent}  'python/rurex-bytes',\n")
                if insert:
                    lines[i:j] = block[:end] + insert + block[end:]
                    j = i + len(block) + len(insert)
                    additions += len(insert)
        i = j
    return "".join(lines), additions


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]) / "benchmarks" / "definitions"
    files = added = 0
    for path in sorted(root.rglob("*.toml")):
        text = path.read_text()
        new, n = patch_file(path, text)
        if n:
            path.write_text(new)
            files += 1
            added += n
    print(f"patched {files} definition files, added {added} engine entries")


if __name__ == "__main__":
    main()
