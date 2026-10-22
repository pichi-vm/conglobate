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
stage = Path('/guardian-watch-stage').read_text().strip()
helper = None
helper_fd = None
abort = None


def checkpoint(name):
    if stage != name:
        return False
    Path('/work/guardian-watch-stage-ready.tmp').write_text(name)
    Path('/work/guardian-watch-stage-ready.tmp').replace('/work/guardian-watch-stage-ready')
    assert poll.poll(10000), 'guardian-loss checkpoint expired'
    return True


lost = bool(poll.poll(0))
if stage == 'before-watch-registration':
    assert lost, 'guardian must already be dead when the monitor starts'
    print('GUARDIAN_WATCH_ALREADY_DEAD_REJECTED_PASS', flush=True)
lost = lost or checkpoint('before-launch')
if not lost:
    helper = subprocess.Popen(['/bin/bash', '-c',
        'kill -STOP $$; exec unshare --mount /publication-helper --publish'])
    helper_fd = os.pidfd_open(helper.pid)
    pid, status = os.waitpid(helper.pid, os.WUNTRACED)
    assert pid == helper.pid and os.WIFSTOPPED(status)
    Path('/work/helper-started').write_text(str(helper.pid))
    lost = checkpoint('before-group-assignment')
if not lost:
    (group / 'cgroup.procs').write_text(str(helper.pid))
    lost = checkpoint('before-resume')
if not lost:
    assert stage == 'active'
    signal.pidfd_send_signal(helper_fd, signal.SIGCONT)
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
    assert abort is not None, 'active case requires a filesystem before loss'
else:
    assert not list(Path('/sys/fs/fuse/connections').iterdir())
    assert not Path('/work/writers-progress').exists()
print('GUARDIAN_WATCH_DEATH_OBSERVED', flush=True)
start = time.monotonic()
# A stopped child may not yet belong to the cgroup; its pidfd remains owned.
if helper_fd is not None:
    signal.pidfd_send_signal(helper_fd, signal.SIGKILL)
(group / 'cgroup.kill').write_text('1')
# Abort before waiting for processes potentially blocked in filesystem calls.
if abort is not None:
    os.write(abort, b'1')
    os.close(abort)
if helper is not None:
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
if helper_fd is not None:
    os.close(helper_fd)
os.close(guardian_fd)
while list(Path('/sys/fs/fuse/connections').iterdir()):
    assert time.monotonic() - start < 10
    time.sleep(.01)
assert not Path('/work/verified').exists()
assert not Path('/work/published').exists()
print('GUARDIAN_WATCH_MONITOR_CLEANUP_PASS', flush=True)
print('GUARDIAN_WATCH_STAGE_PASS', stage, flush=True)
Path('/work/guardian-watch-complete').write_text('failed build; no publication\n')
