"""Exclusive lease for the first four physical cores.

Benchmarks that use the designated physical cores must acquire
`/tmp/core4.lock` before the warm-up and hold it through the whole
matrix. The lock is a host-local `flock`; it does not coordinate across
machines, which is correct because cores are per host.

The owner metadata is written into the lock file after acquisition:

    {"user": ..., "pid": ..., "host": ..., "cpus": [...],
     "start": ..., "command": ...}

A busy lock prints that metadata and waits up to the timeout. On timeout
the process exits without running the benchmark. The kernel releases the
flock automatically on crash or exit; stale metadata is harmless and is
overwritten by the next owner.

The same module can wrap a non-Python benchmark:

    python benchmarks/corelock.py -- cargo run --release ...
"""

import argparse
import fcntl
import getpass
import json
import os
import pathlib
import shlex
import socket
import subprocess
import sys
import time
from datetime import datetime, timezone

CORE_LOCK_PATH = "/tmp/core4.lock"
HELD_ENV = "RUREX_CORE_LOCK_HELD"
TIMEOUT_ENV = "RUREX_CORE_LOCK_TIMEOUT"
DEFAULT_TIMEOUT_S = 600.0
DEFAULT_POLL_S = 1.0


def _default_cpus():
    try:
        from measurement import pick_physical_cpus

        return pick_physical_cpus(4)[0]
    except Exception:
        return list(range(min(4, os.cpu_count() or 1)))


def _owner_from_fd(fd):
    try:
        os.lseek(fd, 0, os.SEEK_SET)
        raw = os.read(fd, 1 << 16)
    except OSError:
        return None
    if not raw:
        return None
    try:
        return json.loads(raw.decode("utf-8", "replace"))
    except (ValueError, UnicodeError):
        return raw.decode("utf-8", "replace").strip() or None


def _format_owner(owner):
    if isinstance(owner, dict):
        start = owner.get("start", owner.get("started"))
        command = owner.get("command", owner.get("args"))
        return (
            f"user={owner.get('user')} pid={owner.get('pid')} "
            f"host={owner.get('host')} cpus={owner.get('cpus')} "
            f"start={start} command={command}"
        )
    if owner:
        return str(owner)
    return "<no metadata>"


class CoreLock:
    """A context manager holding the exclusive core lease."""

    def __init__(
        self,
        cpus=None,
        command=None,
        path=CORE_LOCK_PATH,
        timeout=None,
        poll=DEFAULT_POLL_S,
    ):
        self.path = pathlib.Path(path)
        self.cpus = list(cpus) if cpus is not None else _default_cpus()
        self.command = (
            list(command)
            if command is not None
            else [shlex.join(sys.argv)]
        )
        if timeout is None:
            raw = os.environ.get(TIMEOUT_ENV)
            try:
                timeout = float(raw) if raw is not None else DEFAULT_TIMEOUT_S
            except ValueError:
                timeout = DEFAULT_TIMEOUT_S
        self.timeout = max(0.0, float(timeout))
        self.poll = max(0.05, float(poll))
        self.fd = None

    def __enter__(self):
        self.acquire()
        return self

    def __exit__(self, exc_type, exc, tb):
        self.release()
        return False

    def _write_metadata(self):
        meta = {
            "user": getpass.getuser(),
            "pid": os.getpid(),
            "host": socket.gethostname(),
            "cpus": self.cpus,
            "start": datetime.now(timezone.utc).isoformat(),
            "command": shlex.join(self.command),
        }
        data = json.dumps(meta, ensure_ascii=False).encode("utf-8")
        os.lseek(self.fd, 0, os.SEEK_SET)
        os.ftruncate(self.fd, 0)
        os.write(self.fd, data)
        os.fsync(self.fd)

    def acquire(self):
        if os.environ.get(HELD_ENV):
            return self
        while True:
            try:
                fd = os.open(self.path, os.O_RDWR | os.O_CREAT, 0o666)
            except OSError as exc:
                raise SystemExit(f"cannot open core lock {self.path}: {exc}")
            try:
                os.chmod(self.path, 0o666)
            except OSError:
                pass
            deadline = time.monotonic() + self.timeout
            printed = False
            try:
                while True:
                    try:
                        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        owner = _owner_from_fd(fd)
                        if not printed:
                            print(
                                f"core lock busy: {_format_owner(owner)} "
                                f"(waiting up to {self.timeout:.1f}s)",
                                file=sys.stderr,
                                flush=True,
                            )
                            printed = True
                        if time.monotonic() >= deadline:
                            print(
                                f"core lock timeout: {_format_owner(owner)}",
                                file=sys.stderr,
                                flush=True,
                            )
                            raise SystemExit(
                                f"could not acquire {self.path} within "
                                f"{self.timeout:.0f}s"
                            )
                        time.sleep(self.poll)
                # /tmp cleanup can unlink and recreate the path while we
                # wait. If that happened, the flock is on a dead inode.
                try:
                    opened = os.fstat(fd)
                    current = os.stat(self.path)
                except OSError:
                    fcntl.flock(fd, fcntl.LOCK_UN)
                    os.close(fd)
                    continue
                if (opened.st_dev, opened.st_ino) != (
                    current.st_dev,
                    current.st_ino,
                ):
                    fcntl.flock(fd, fcntl.LOCK_UN)
                    os.close(fd)
                    continue
            except BaseException:
                try:
                    fcntl.flock(fd, fcntl.LOCK_UN)
                except OSError:
                    pass
                os.close(fd)
                raise
            self.fd = fd
            self._write_metadata()
            print(
                f"core lock acquired: cpus={self.cpus} pid={os.getpid()}",
                file=sys.stderr,
                flush=True,
            )
            return self

    def release(self):
        if self.fd is None:
            return
        try:
            fcntl.flock(self.fd, fcntl.LOCK_UN)
        finally:
            os.close(self.fd)
            self.fd = None


def run_command(command, cpus=None, timeout=None, path=CORE_LOCK_PATH):
    """Hold the lease while a non-Python benchmark command runs."""
    with CoreLock(cpus=cpus, command=command, timeout=timeout, path=path):
        env = dict(os.environ)
        env[HELD_ENV] = "1"
        return subprocess.run(command, env=env).returncode


def main():
    argv = sys.argv[1:]
    opts = argv
    command = []
    if "--" in argv:
        i = argv.index("--")
        opts, command = argv[:i], argv[i + 1 :]
    ap = argparse.ArgumentParser()
    ap.add_argument("--timeout", type=float, default=None)
    ap.add_argument("--cpus", default="")
    ns = ap.parse_args(opts)
    if not command:
        ap.error("a command is required after --")
    cpus = [int(x) for x in ns.cpus.split(",") if x.strip()] or None
    if cpus is None:
        cpus = _default_cpus()
    sys.exit(run_command(command, cpus=cpus, timeout=ns.timeout))


if __name__ == "__main__":
    main()
