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
owner = subprocess.Popen(['/usr/bin/python3', '/admission.py'])
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
    signal.pidfd_send_signal(handles['coordinator'], signal.SIGKILL)
    assert owner.wait(timeout=5) == -signal.SIGKILL
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
    print('COORDINATOR_DEATH_STORAGE_BASE_AND_NO_OUTPUT_PASS', flush=True)
finally:
    # Failure-only containment; cannot manufacture the success markers above.
    for fd in handles.values():
        if not terminal(fd):
            signal.pidfd_send_signal(fd, signal.SIGKILL)
    owner.wait(timeout=5)
    for fd in handles.values():
        os.close(fd)
