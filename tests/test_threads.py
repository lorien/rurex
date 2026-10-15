"""Concurrency smoke tests: the GIL-release (detach) paths must not
deadlock or corrupt results under threads."""

from concurrent.futures import ThreadPoolExecutor

import rurex


def test_concurrent_findall_shared_regex():
    text = "abc123 " * 2000
    rx = rurex.compile(r"(\w+)(\d+)")
    expected = rx.findall(text)
    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(lambda _: rx.findall(text), range(8)))
    assert all(r == expected for r in results)


def test_concurrent_module_functions_share_cache():
    text = "abc123 " * 2000
    expected = rurex.findall(r"(\w+)(\d+)", text)
    with ThreadPoolExecutor(max_workers=4) as pool:
        futures = [
            pool.submit(rurex.findall, r"(\w+)(\d+)", text) for _ in range(8)
        ]
        results = [f.result() for f in futures]
    assert all(r == expected for r in results)


def test_concurrent_long_scan_is_parallelizable():
    # A long scan releases the GIL; other threads must make progress
    # while it runs.
    big = "x" * 2_000_000 + "needle"
    with ThreadPoolExecutor(max_workers=2) as pool:
        scan = pool.submit(rurex.search, r"needle", big)
        spinner = pool.submit(rurex.findall, r"\d+", "1 " * 10_000)
        assert scan.result().span() == (2_000_000, 2_000_006)
        assert len(spinner.result()) == 10_000
