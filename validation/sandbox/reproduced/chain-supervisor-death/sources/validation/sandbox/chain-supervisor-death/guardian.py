#!/usr/bin/python3
"""Disposable-VM trusted guardian for a single real storage-chain build."""
import ctypes
import os
from pathlib import Path
import select
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
    raise AssertionError('guardian condition timed out')

def names():
    output = subprocess.check_output(['dmsetup', 'info', '-c', '--noheadings', '-o', 'name'], text=True)
    return set() if output.strip() == "No devices found" else set(output.split())

assert not names(), 'fixture requires an empty disposable device namespace'
assert not subprocess.check_output(['losetup', '-a'])
group = Path('/sys/fs/cgroup/chain-fixture')
group.mkdir()
# Stop before creating any descendants. unshare execs the helper in a private
# mount namespace; all command/worker descendants inherit cgroup membership.
child = subprocess.Popen(['/bin/bash', '-c', 'kill -STOP $$; exec unshare --mount /publication-helper --publish'])
pidfd = os.pidfd_open(child.pid)
pid, status = os.waitpid(child.pid, os.WUNTRACED)
assert pid == child.pid and os.WIFSTOPPED(status)
(group / 'cgroup.procs').write_text(str(child.pid))
signal.pidfd_send_signal(pidfd, signal.SIGCONT)
until(lambda: Path('/work/writers-ready').exists(), 60)
owned = names()
assert owned, 'no real storage devices assembled'
connections = list(Path('/sys/fs/fuse/connections').iterdir())
assert len(connections) == 1, connections
abort = os.open(connections[0] / 'abort', os.O_WRONLY)
# Initial supervisor death must be observed before guardian cleanup starts.
signal.pidfd_send_signal(pidfd, signal.SIGKILL)
poll = select.poll()
poll.register(pidfd, select.POLLIN)
assert poll.poll(2000)
assert child.wait(timeout=2) == -signal.SIGKILL
print('SUPERVISOR_DEATH_OBSERVED', flush=True)
start = time.monotonic()
(group / 'cgroup.kill').write_text('1')
# Do not depend on a killed filesystem daemon servicing queued requests.
os.write(abort, b'1')
os.close(abort)
until(lambda: 'populated 0' in (group / 'cgroup.events').read_text())
while True:
    try:
        pid, _ = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert time.monotonic() - start < 10, 'descendant reaping timed out'
    if pid == 0:
        time.sleep(.01)
assert (group / 'cgroup.procs').read_text() == ''
group.rmdir()
os.close(pidfd)
until(lambda: not list(Path('/sys/fs/fuse/connections').iterdir()))
remaining = names()
assert remaining <= owned
print('SUPERVISOR_DEATH_PROCESSES_AND_FUSE_RELEASED', flush=True)
print('SUPERVISOR_DEATH_REMAINING_DM', sorted(remaining), flush=True)
# Explicit recovery of exactly the names captured for this one isolated build.
# No remove_all: concurrent ownership/reused identities require further design.
for _ in range(len(owned) + 1):
    for name in sorted(remaining):
        subprocess.run(['dmsetup', 'remove', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    remaining = names()
    assert remaining <= owned
    if not remaining:
        break
assert not remaining, remaining
until(lambda: not subprocess.check_output(['losetup', '-a']))
assert not Path('/work/published').exists()
assert not Path('/work/verified').exists()
assert time.monotonic() - start < 10
print('SUPERVISOR_DEATH_OWNED_DEVICE_RECOVERY_PASS', flush=True)
