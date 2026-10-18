"""Tests for the /tmp/core4.lock benchmark lease."""

import json
import multiprocessing
import os
import pathlib
import stat
import sys
import time

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "benchmarks"))
import corelock  # noqa: E402


def _hold(path, ready):
    with corelock.CoreLock(path=path, cpus=[0], command=["holder"], timeout=1):
        ready.set()
        time.sleep(5)


def test_metadata_mode_and_release(tmp_path):
    path = tmp_path / "core.lock"
    with corelock.CoreLock(path=path, cpus=[0, 2, 4, 6], command=["bench", "--x"]):
        assert stat.S_IMODE(path.stat().st_mode) == 0o666
        meta = json.loads(path.read_text())
        assert meta["pid"] == os.getpid()
        assert meta["cpus"] == [0, 2, 4, 6]
        assert meta["command"] == "bench --x"
        assert meta["start"]
    # The lease must be free again.
    with corelock.CoreLock(path=path, cpus=[0], command=["again"]):
        pass


def test_stale_metadata_is_overwritten(tmp_path):
    path = tmp_path / "core.lock"
    path.write_text("pid=1 host=old user=old cpus=0 started=old args=old\n")
    with corelock.CoreLock(path=path, cpus=[0], command=["new"]):
        meta = json.loads(path.read_text())
        assert meta["command"] == "new"
        assert meta["pid"] == os.getpid()


def test_busy_timeout_and_owner(tmp_path, capsys):
    path = str(tmp_path / "core.lock")
    ctx = multiprocessing.get_context("fork")
    ready = ctx.Event()
    proc = ctx.Process(target=_hold, args=(path, ready))
    proc.start()
    try:
        assert ready.wait(5)
        with pytest.raises(SystemExit):
            corelock.CoreLock(
                path=path,
                cpus=[1],
                command=["try"],
                timeout=0.2,
                poll=0.05,
            ).acquire()
        err = capsys.readouterr().err
        assert "core lock busy" in err
        assert "holder" in err
    finally:
        proc.terminate()
        proc.join(5)


def test_release_on_exception(tmp_path):
    path = tmp_path / "core.lock"
    with pytest.raises(RuntimeError):
        with corelock.CoreLock(path=path, cpus=[0], command=["boom"]):
            raise RuntimeError("boom")
    with corelock.CoreLock(path=path, cpus=[0], command=["after"]):
        pass


def test_inherited_marker_skips_lock(tmp_path, monkeypatch):
    path = tmp_path / "core.lock"
    monkeypatch.setenv(corelock.HELD_ENV, "1")
    with corelock.CoreLock(path=path, cpus=[0], command=["child"]):
        pass
    assert not path.exists()


def test_text_owner_format():
    text = "pid=1 host=h user=u cpus=0,2,4,6 started=old args=cmd"
    assert "pid=1" in corelock._format_owner(text)
    assert "start=old" in corelock._format_owner(
        {"pid": 1, "started": "old", "args": "cmd"}
    )


def test_run_command_wrapper(tmp_path):
    path = tmp_path / "core.lock"
    rc = corelock.run_command(
        [sys.executable, "-c", "raise SystemExit(0)"],
        cpus=[0],
        path=path,
    )
    assert rc == 0
    assert path.exists()
