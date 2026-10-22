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
Path('/sys/fs/cgroup/cgroup.subtree_control').write_text('+memory')
group.mkdir()
(group / 'cgroup.subtree_control').write_text('+memory')
supervisor = group / 'supervisor'
workers = group / 'workers'
supervisor.mkdir()
workers.mkdir()
(supervisor / 'memory.swap.max').write_text('0')
# Stop before creating any descendants. unshare execs the helper in a private
# mount namespace; all command/worker descendants inherit cgroup membership.
child = subprocess.Popen(['/bin/bash', '-c', 'kill -STOP $$; exec unshare --mount /publication-helper --publish'], env=dict(os.environ, CHAIN_WORKER_LAUNCHER='/worker-dispatch'))
pidfd = os.pidfd_open(child.pid)
pid, status = os.waitpid(child.pid, os.WUNTRACED)
assert pid == child.pid and os.WIFSTOPPED(status)
(supervisor / 'cgroup.procs').write_text(str(child.pid))
signal.pidfd_send_signal(pidfd, signal.SIGCONT)
until(lambda: Path('/work/writers-ready').exists(), 60)
observed_devices = names()
assert observed_devices, 'no real storage devices assembled'
# Independent trusted read-only consumer, outside the doomed cgroup. These
# are the base carapace mappings, not the build's writable snapshot target.
consumers = {}
for name in sorted(observed_devices):
    if not name.startswith('CARAPACE-'):
        continue
    minor = int(subprocess.check_output(['dmsetup', 'info', '-c', '--noheadings', '-o', 'minor', name], text=True).strip())
    fd = os.open(f'/dev/dm-{minor}', os.O_RDONLY)
    data = os.pread(fd, 4096, 0)
    assert len(data) == 4096
    consumers[name] = (fd, os.fstat(fd).st_rdev, data)
assert 'CARAPACE-z' in consumers and len(consumers) >= 3, consumers.keys()
print('SUPERVISOR_DEATH_SHARED_CONSUMERS_PINNED', sorted(consumers), flush=True)
connections = list(Path('/sys/fs/fuse/connections').iterdir())
assert len(connections) == 1, connections
abort = os.open(connections[0] / 'abort', os.O_WRONLY)
# Initial supervisor death must be observed before guardian cleanup starts.
assert (supervisor / 'cgroup.procs').read_text().split() == [str(child.pid)]
assert (workers / 'cgroup.procs').read_text().strip()
assert int((supervisor / 'memory.current').read_text()) > 0
print('SUPERVISOR_OOM_ONLY_SUPERVISOR_CONSTRAINED', flush=True)
# Kernel memory-controller event, not a synthetic SIGKILL to the supervisor.
(supervisor / 'memory.max').write_text('0')
def oom_killed():
    events = dict(line.split() for line in (supervisor / 'memory.events').read_text().splitlines())
    return int(events['oom_kill']) >= 1
until(oom_killed)
print('SUPERVISOR_OOM_KERNEL_KILL_CONFIRMED', flush=True)
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
    assert time.monotonic() - start < 20, 'descendant reaping timed out'
    if pid == 0:
        time.sleep(.01)
assert (group / 'cgroup.procs').read_text() == ''
supervisor.rmdir()
workers.rmdir()
group.rmdir()
os.close(pidfd)
until(lambda: not list(Path('/sys/fs/fuse/connections').iterdir()))
remaining = names()
assert remaining <= observed_devices
print('SUPERVISOR_DEATH_PROCESSES_AND_FUSE_RELEASED', flush=True)
print('SUPERVISOR_DEATH_REMAINING_DM', sorted(remaining), flush=True)
# Deferred removal must preserve the independently held base mappings after
# every failed-build process and writable-snapshot holder has disappeared.
until(lambda: names() == set(consumers))
for name, (fd, identity, data) in consumers.items():
    assert os.fstat(fd).st_rdev == identity
    assert os.pread(fd, 4096, 0) == data, name
assert subprocess.check_output(['losetup', '-a']), 'base loop backing released too early'
print('SUPERVISOR_DEATH_SHARED_CONSUMERS_SURVIVE', flush=True)
for fd, _, _ in consumers.values():
    os.close(fd)
# Carapace schedules deferred removal before returning each mapping. Kernel
# cascading removal may still be queued after all userspace processes exit.
until(lambda: not names())
print('SUPERVISOR_DEATH_DEFERRED_DEVICE_RELEASE_PASS', flush=True)
until(lambda: not subprocess.check_output(['losetup', '-a']))
assert not Path('/work/published').exists()
assert not Path('/work/verified').exists()
assert time.monotonic() - start < 20
print('SUPERVISOR_DEATH_OWNED_DEVICE_RECOVERY_PASS', flush=True)
