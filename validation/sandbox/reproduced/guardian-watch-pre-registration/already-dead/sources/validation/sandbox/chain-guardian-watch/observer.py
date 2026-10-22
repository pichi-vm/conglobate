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
monitor_fd = None
try:
    stage = Path('/guardian-watch-stage').read_text().strip()
    if stage == 'before-watch-registration':
        ready = Path('/work/monitor-before-registration')
        until(lambda: ready.exists())
        monitor_pid = int(ready.read_text())
        monitor_fd = os.pidfd_open(monitor_pid)
        assert '\nState:\tT' in Path(f'/proc/{monitor_pid}/status').read_text()
        assert not Path('/sys/fs/cgroup/guardian-watch-fixture').exists()
        assert not Path('/work/helper-started').exists()
    elif stage == 'active':
        until(lambda: Path('/work/guardian-watch-armed').exists()
              and Path('/work/writers-progress').exists(), 60)
        before = [int(value) for value in Path('/work/writers-progress').read_text().split()]
        assert len(before) == 2 and min(before) > 0
        time.sleep(.1)
        after = [int(value) for value in Path('/work/writers-progress').read_text().split()]
        assert all(a > b for a, b in zip(after, before))
    else:
        ready = Path('/work/guardian-watch-stage-ready')
        until(lambda: ready.exists())
        assert ready.read_text() == stage
        children = Path('/sys/fs/cgroup/guardian-watch-fixture/cgroup.procs').read_text().split()
        started = Path('/work/helper-started')
        if stage == 'before-launch':
            assert not started.exists() and not children
        else:
            helper_pid = int(started.read_text())
            assert '\nState:\tT' in Path(f'/proc/{helper_pid}/status').read_text()
            assert Path(f'/proc/{helper_pid}/task/{helper_pid}/children').read_text() == ''
            assert children == ([str(helper_pid)] if stage == 'before-resume' else [])
        assert not Path('/work/writers-progress').exists()
    assert guardian.poll() is None
    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
    assert guardian.wait(timeout=5) == -signal.SIGKILL
    if monitor_fd is not None:
        signal.pidfd_send_signal(monitor_fd, signal.SIGCONT)
    until(lambda: Path('/work/guardian-watch-complete').exists())
    assert not Path('/sys/fs/cgroup/guardian-watch-fixture').exists()
    assert not list(Path('/sys/fs/fuse/connections').iterdir())
    if stage == 'active':
        progress = Path('/work/writers-progress').read_bytes()
        time.sleep(.1)
        assert Path('/work/writers-progress').read_bytes() == progress
    else:
        assert not Path('/work/writers-progress').exists()
        if stage == 'before-watch-registration':
            assert not Path('/work/helper-started').exists()
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
        if monitor_fd is not None:
            try:
                signal.pidfd_send_signal(monitor_fd, signal.SIGKILL)
            except ProcessLookupError:
                pass
    if monitor_fd is not None:
        os.close(monitor_fd)
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
