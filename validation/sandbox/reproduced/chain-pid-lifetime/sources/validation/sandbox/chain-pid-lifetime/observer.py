#!/usr/bin/python3
"""Test kernel descendant lifetime when the whole-build PID init dies."""
import os
from pathlib import Path
import signal
import subprocess
import time

def until(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('outer PID lifetime deadline')

def children(pid):
    return [int(value) for value in Path(f'/proc/{pid}/task/{pid}/children').read_text().split()]

def fields(pid):
    return dict(line.split(':', 1) for line in Path(f'/proc/{pid}/status').read_text().splitlines())

group = Path('/sys/fs/cgroup/guardian-watch-fixture')
launcher = subprocess.Popen(['unshare', '--mount', '--pid', '--fork', '--mount-proc',
                             'python3', '/guardian.py'])
guardian_fd = None
monitor_fd = None
passed = False
try:
    until(lambda: Path('/work/writers-progress').exists()
          and Path('/work/guardian-watch-armed').exists()
          and Path('/work/watch-monitor-pid').exists(), 60)
    assert launcher.poll() is None
    guardian_pid, = children(launcher.pid)
    assert [int(v) for v in fields(guardian_pid)['NSpid'].split()] == [guardian_pid, 1]
    guardian_fd = os.pidfd_open(guardian_pid)
    monitor_pid, = children(guardian_pid)
    monitor_local = int(Path('/work/watch-monitor-pid').read_text())
    assert [int(v) for v in fields(monitor_pid)['NSpid'].split()] == [monitor_pid, monitor_local]
    monitor_fd = os.pidfd_open(monitor_pid)
    before = [int(v) for v in Path('/work/writers-progress').read_text().split()]
    time.sleep(.1)
    after = [int(v) for v in Path('/work/writers-progress').read_text().split()]
    assert len(after) == 2 and all(a > b for a, b in zip(after, before))
    assert len(list(Path('/sys/fs/fuse/connections').iterdir())) == 1
    assert 'populated 1' in (group / 'cgroup.events').read_text()
    # Monitor cannot service guardian death. No signal goes to the monitor or
    # build after this stop; the kernel must terminate all namespace descendants.
    signal.pidfd_send_signal(monitor_fd, signal.SIGSTOP)
    until(lambda: '\nState:\tT' in Path(f'/proc/{monitor_pid}/status').read_text())
    signal.pidfd_send_signal(guardian_fd, signal.SIGKILL)
    assert launcher.wait(timeout=10) in (137, -signal.SIGKILL)
    until(lambda: 'populated 0' in (group / 'cgroup.events').read_text())
    until(lambda: not list(Path('/sys/fs/fuse/connections').iterdir()))
    assert not Path('/work/guardian-watch-complete').exists()
    progress = Path('/work/writers-progress').read_bytes()
    time.sleep(.1)
    assert Path('/work/writers-progress').read_bytes() == progress
    assert (group / 'cgroup.procs').read_text() == ''
    group.rmdir()  # Empty bookkeeping only; no cgroup.kill or FUSE abort on pass.
    assert not Path('/work/published').exists()
    assert not Path('/work/verified').exists()
    passed = True
    print('OUTER_PID_INIT_DEATH_KERNEL_CLEANUP_PASS', flush=True)
finally:
    if not passed:
        print('OUTER_PID_LIFETIME_HARNESS_FALLBACK_FAILURE', flush=True)
        if group.exists():
            (group / 'cgroup.kill').write_text('1')
        for connection in Path('/sys/fs/fuse/connections').iterdir():
            (connection / 'abort').write_text('1')
        if guardian_fd is not None:
            try:
                signal.pidfd_send_signal(guardian_fd, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if launcher.poll() is None:
            launcher.kill()
            launcher.wait(timeout=5)
    for fd in (guardian_fd, monitor_fd):
        if fd is not None:
            os.close(fd)
