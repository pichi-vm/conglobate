#!/usr/bin/python3
"""Kill only an owned coordinator; observe linked lifetimes in a disposable VM."""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import time

base_hash = hashlib.sha256(Path('/work/base').read_bytes()).hexdigest()
group = Path('/sys/fs/cgroup/admission-pressure')
group.mkdir()
(group / 'memory.swap.max').write_text('0')
(group / 'memory.oom.group').write_text('1')
owner = subprocess.Popen(['/bin/bash', '-c', 'kill -STOP $$; exec /usr/bin/python3 /admission.py'])
deadline = time.monotonic() + 5
while '\nState:\tT' not in Path(f'/proc/{owner.pid}/status').read_text():
    assert owner.poll() is None and time.monotonic() < deadline
    time.sleep(.01)
(group / 'cgroup.procs').write_text(str(owner.pid))
os.kill(owner.pid, signal.SIGCONT)
handles = {'coordinator': os.pidfd_open(owner.pid)}

def terminal(fd, timeout=0):
    poller = select.poll()
    poller.register(fd, select.POLLIN)
    return bool(poller.poll(timeout))

try:
    deadline = time.monotonic() + 90
    record = Path('/work/coordinator-owned-handles.json')
    while not record.exists():
        assert owner.poll() is None and time.monotonic() < deadline
        time.sleep(.01)
    # The writer is trusted but write_text is not atomic; wait for complete JSON.
    while True:
        try:
            identities = json.loads(record.read_text())
            break
        except json.JSONDecodeError:
            assert owner.poll() is None and time.monotonic() < deadline
            time.sleep(.01)
    assert identities['coordinator'] == owner.pid
    for name in ('arbiter', 'launcher', 'init'):
        handles[name] = os.pidfd_open(identities[name])
        status = Path(f'/proc/{identities[name]}/status').read_text()
        parent = int(status.split('PPid:')[1].splitlines()[0])
        assert parent == identities['launcher' if name == 'init' else 'coordinator']
        assert not terminal(handles[name])
    assert not terminal(handles['coordinator'])
    print('LIVE_COORDINATOR_CHILD_HANDLES_PINNED_PASS', flush=True)
    members = {int(pid) for pid in (group / 'cgroup.procs').read_text().split()}
    assert set(identities.values()) <= members
    assert os.getpid() not in members
    assert int((group / 'memory.current').read_text()) > 0
    before = dict(line.split() for line in (group / 'memory.events').read_text().splitlines())
    (group / 'memory.max').write_text('0')
    assert owner.wait(timeout=5) == -signal.SIGKILL
    after = dict(line.split() for line in (group / 'memory.events').read_text().splitlines())
    assert int(after['oom_kill']) > int(before['oom_kill'])
    assert int(after['oom_group_kill']) > int(before['oom_group_kill'])
    print('COORDINATOR_GROUP_OOM_KERNEL_EVENTS_PASS', json.dumps(after, sort_keys=True), flush=True)
    if Path('/admission-mode').read_text().strip() == 'owner-unlinked':
        time.sleep(.1)
        for name in ('arbiter', 'launcher', 'init'):
            assert not terminal(handles[name]), (name, 'negative control unexpectedly terminal')
        print('UNLINKED_COORDINATOR_CHILDREN_SURVIVE_NEGATIVE_CONTROL', flush=True)
        # Explicit fixture cleanup in the known-negative case, not automatic teardown.
        for name in ('arbiter', 'launcher', 'init'):
            signal.pidfd_send_signal(handles[name], signal.SIGKILL)
    for name in ('arbiter', 'launcher', 'init'):
        assert terminal(handles[name], 5000), (name, 'survived coordinator death')
    if Path('/admission-mode').read_text().strip() == 'owner-death':
        print('COORDINATOR_DEATH_LINKED_LIFETIMES_TERMINAL_PASS', flush=True)
    deadline = time.monotonic() + 5
    while True:
        dm = subprocess.check_output(['dmsetup', 'ls'], timeout=2).decode().strip()
        loops = subprocess.check_output(['losetup', '-a'], timeout=2).decode().strip()
        fuse = list(Path('/sys/fs/fuse/connections').iterdir())
        if dm == 'No devices found' and not loops and not fuse:
            break
        assert time.monotonic() < deadline, (dm, loops, fuse)
        time.sleep(.02)
    assert hashlib.sha256(Path('/work/base').read_bytes()).hexdigest() == base_hash
    for path in ('/work/published', '/work/admission-control/published', '/work/admission-control/result'):
        assert not Path(path).exists()
    deadline = time.monotonic() + 5
    while (group / 'cgroup.procs').read_text().split():
        assert time.monotonic() < deadline
        time.sleep(.01)
    assert 'populated 0' in (group / 'cgroup.events').read_text()
    group.rmdir()
    print('COORDINATOR_GROUP_OOM_STORAGE_BASE_AND_NO_OUTPUT_PASS', flush=True)
finally:
    # Failure-only containment; cannot manufacture the success markers above.
    for fd in handles.values():
        if not terminal(fd):
            signal.pidfd_send_signal(fd, signal.SIGKILL)
    owner.wait(timeout=5)
    for fd in handles.values():
        os.close(fd)
