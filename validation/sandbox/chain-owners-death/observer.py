#!/usr/bin/python3
"""Independent fixture owner observes guardian death and bounds its cleanup."""
import ctypes
import os
from pathlib import Path
import signal
import select
import subprocess
import time

assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0


def until(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('guardian-death fixture deadline')


group = Path('/sys/fs/cgroup/guardian-watch-fixture')
guardian = subprocess.Popen(['python3', '/guardian.py'])
guardian_fd = os.pidfd_open(guardian.pid)
abort = None
monitor_fd = None
try:
    until(lambda: Path('/work/writers-progress').exists() and Path('/work/guardian-watch-armed').exists() and Path('/work/watch-monitor-pid').exists(), 60)
    assert guardian.poll() is None
    connections = list(Path('/sys/fs/fuse/connections').iterdir())
    assert len(connections) == 1
    abort = os.open(connections[0] / 'abort', os.O_WRONLY)

    def sizes():
        values = [int(value) for value in Path('/work/writers-progress').read_text().split()]
        assert len(values) == 2
        return values

    before = sizes()
    assert min(before) > 0
    monitor_pid = int(Path('/work/watch-monitor-pid').read_text())
    fields = dict(line.split(':', 1) for line in Path(f'/proc/{monitor_pid}/status').read_text().splitlines())
    assert int(fields['PPid']) == guardian.pid
    monitor_fd = os.pidfd_open(monitor_pid)
    # Stop the monitor before guardian loss, preventing it from handling the
    # notification before its own death. This is one ordered overlapping loss.
    signal.pidfd_send_signal(monitor_fd, signal.SIGSTOP)
    until(lambda: '\nState:\tT' in Path(f'/proc/{monitor_pid}/status').read_text())
    signal.pidfd_send_signal(guardian_fd, signal.SIGKILL)
    assert guardian.wait(timeout=5) == -signal.SIGKILL
    signal.pidfd_send_signal(monitor_fd, signal.SIGKILL)
    poll = select.poll()
    poll.register(monitor_fd, select.POLLIN)
    assert poll.poll(2000)
    assert not Path('/work/guardian-watch-complete').exists()
    until(lambda: all(after > previous for after, previous in zip(sizes(), before)), 3)
    assert 'populated 1' in (group / 'cgroup.events').read_text()
    print('BOTH_OWNERS_LOST_WRITERS_CONTINUE_LIMITATION', flush=True)
finally:
    # Test harness cleanup is outside both guardian and build cgroups. It is
    # not evidence that the candidate supervisor detected guardian failure.
    if group.exists():
        (group / 'cgroup.kill').write_text('1')
    if abort is not None:
        os.write(abort, b'1')
        os.close(abort)
    if guardian.poll() is None:
        signal.pidfd_send_signal(guardian_fd, signal.SIGKILL)
        guardian.wait(timeout=5)
    if monitor_fd is not None:
        try:
            signal.pidfd_send_signal(monitor_fd, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.close(monitor_fd)
    os.close(guardian_fd)
until(lambda: 'populated 0' in (group / 'cgroup.events').read_text())
deadline = time.monotonic() + 10
while True:
    try:
        pid, _ = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert time.monotonic() < deadline
    if pid == 0:
        time.sleep(.01)
assert (group / 'cgroup.procs').read_text() == ''
group.rmdir()
until(lambda: not list(Path('/sys/fs/fuse/connections').iterdir()))
assert not Path('/work/published').exists()
assert not Path('/work/verified').exists()
print('BOTH_OWNERS_LOST_HARNESS_CLEANUP_PASS', flush=True)
