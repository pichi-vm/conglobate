#!/usr/bin/python3
"""Inject guardian death; require the candidate monitor to perform cleanup."""
import ctypes
import os
from pathlib import Path
import signal
import subprocess
import time

assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0


def until(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('guardian-watch fixture deadline')


guardian = subprocess.Popen(['python3', '/guardian.py'])
pidfd = os.pidfd_open(guardian.pid)
passed = False
try:
    until(lambda: Path('/work/guardian-watch-armed').exists()
          and Path('/work/writers-progress').exists(), 60)
    assert guardian.poll() is None
    before = [int(value) for value in Path('/work/writers-progress').read_text().split()]
    assert len(before) == 2 and min(before) > 0
    time.sleep(.1)
    after = [int(value) for value in Path('/work/writers-progress').read_text().split()]
    assert all(a > b for a, b in zip(after, before))
    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
    assert guardian.wait(timeout=5) == -signal.SIGKILL
    until(lambda: Path('/work/guardian-watch-complete').exists())
    assert not Path('/sys/fs/cgroup/guardian-watch-fixture').exists()
    assert not list(Path('/sys/fs/fuse/connections').iterdir())
    progress = Path('/work/writers-progress').read_bytes()
    time.sleep(.1)
    assert Path('/work/writers-progress').read_bytes() == progress
    passed = True
finally:
    if not passed:
        # Bounded harness fallback makes a failed experiment safe, but cannot
        # produce the required monitor-completion or no-observer-cleanup pass.
        group = Path('/sys/fs/cgroup/guardian-watch-fixture')
        if group.exists():
            (group / 'cgroup.kill').write_text('1')
        for connection in Path('/sys/fs/fuse/connections').iterdir():
            (connection / 'abort').write_text('1')
        if guardian.poll() is None:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            guardian.wait(timeout=5)
    os.close(pidfd)
deadline = time.monotonic() + 5
while True:
    try:
        pid, status = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert time.monotonic() < deadline
    if pid == 0:
        time.sleep(.01)
    else:
        assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0
print('GUARDIAN_WATCH_NO_OBSERVER_CLEANUP_PASS', flush=True)
