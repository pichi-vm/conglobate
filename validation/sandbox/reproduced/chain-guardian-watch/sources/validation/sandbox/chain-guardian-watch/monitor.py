#!/usr/bin/python3
"""Candidate trusted supervisor-side guardian watch, outside the build cgroup."""
import ctypes
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
guardian_fd = int(sys.argv[1])
poll = select.poll()
poll.register(guardian_fd, select.POLLIN)
group = Path('/sys/fs/cgroup/guardian-watch-fixture')
group.mkdir()
helper = subprocess.Popen(['/bin/bash', '-c',
    'kill -STOP $$; exec unshare --mount /publication-helper --publish'])
helper_fd = os.pidfd_open(helper.pid)
pid, status = os.waitpid(helper.pid, os.WUNTRACED)
assert pid == helper.pid and os.WIFSTOPPED(status)
(group / 'cgroup.procs').write_text(str(helper.pid))
signal.pidfd_send_signal(helper_fd, signal.SIGCONT)
abort = None
deadline = time.monotonic() + 70
while not poll.poll(20):
    assert helper.poll() is None, 'build ended before guardian-loss test'
    assert time.monotonic() < deadline
    if abort is None:
        connections = list(Path('/sys/fs/fuse/connections').iterdir())
        assert len(connections) <= 1, connections
        if connections:
            abort = os.open(connections[0] / 'abort', os.O_WRONLY)
            Path('/work/guardian-watch-armed').touch()
assert abort is not None, 'this case requires an active filesystem before loss'
print('GUARDIAN_WATCH_DEATH_OBSERVED', flush=True)
start = time.monotonic()
(group / 'cgroup.kill').write_text('1')
# Abort before waiting for processes potentially blocked in filesystem calls.
os.write(abort, b'1')
os.close(abort)
assert helper.wait(timeout=5) == -signal.SIGKILL
while True:
    try:
        pid, _ = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert time.monotonic() - start < 10
    if pid == 0:
        time.sleep(.01)
assert 'populated 0' in (group / 'cgroup.events').read_text()
assert (group / 'cgroup.procs').read_text() == ''
group.rmdir()
os.close(helper_fd)
os.close(guardian_fd)
while list(Path('/sys/fs/fuse/connections').iterdir()):
    assert time.monotonic() - start < 10
    time.sleep(.01)
assert not Path('/work/verified').exists()
assert not Path('/work/published').exists()
print('GUARDIAN_WATCH_MONITOR_CLEANUP_PASS', flush=True)
Path('/work/guardian-watch-complete').write_text('failed build; no publication\n')
