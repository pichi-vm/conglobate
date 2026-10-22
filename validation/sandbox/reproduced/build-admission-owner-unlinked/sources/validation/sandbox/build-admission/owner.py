#!/usr/bin/python3
"""Trusted diagnostic coordinator; no recipe logs authorize publication."""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import time

root = Path('/work/admission-control')
root.mkdir(mode=0o700)
mode = Path('/admission-mode').read_text().strip()
assert mode in ('owner-death', 'owner-unlinked')
linked = mode == 'owner-death'
parent_fd = os.pidfd_open(os.getpid())
arbiter = subprocess.Popen((['/owned-exec'] if linked else []) + ['/commit-arbiter', str(root), '--await-completion'], stdin=parent_fd if linked else subprocess.DEVNULL)
arbiter_fd = os.pidfd_open(arbiter.pid)
launcher = None
launcher_fd = None
init_fd = None
base_hash = hashlib.sha256(Path('/work/base').read_bytes()).hexdigest()

def rpc(operation):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(2)
        stream.connect(str(root / 'control.sock'))
        stream.sendall(operation.encode() + b'\n')
        reply = stream.makefile('rb').readline(64)
        assert reply.endswith(b'\n')
        return reply.decode().strip()

def hashes(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in path.rglob('*') if p.is_file()}

try:
    deadline = time.monotonic() + 5
    while not (root / 'control.sock').exists():
        assert arbiter.poll() is None and time.monotonic() < deadline
        time.sleep(.01)
    assert rpc('status') == 'Building'
    launcher = subprocess.Popen((['/owned-exec'] if linked else []) + ['/usr/bin/python3', '/outer-launcher.py'], stdin=parent_fd if linked else subprocess.DEVNULL)
    launcher_fd = os.pidfd_open(launcher.pid)
    deadline = time.monotonic() + 90
    while not Path('/work/admission-active').exists():
        assert launcher.poll() is None and time.monotonic() < deadline
        time.sleep(.01)
    # This owned marker only schedules the experiment. It never authorizes commit.
    assert rpc('reserve') == 'Building'
    assert rpc('commit') == 'Building'
    assert not (root / 'result').exists()
    print('LIVE_BUILD_PREMATURE_COMMIT_REJECTED_PASS', flush=True)
    # Pin the exact live namespace init before killing its dedicated parent.
    children = Path(f'/proc/{launcher.pid}/task/{launcher.pid}/children').read_text().split()
    init_pid, = children
    init_fd = os.pidfd_open(int(init_pid))
    status = Path(f'/proc/{init_pid}/status').read_text()
    assert status.split('NSpid:')[1].splitlines()[0].split() == [init_pid, '1']
    init_poll = select.poll()
    init_poll.register(init_fd, select.POLLIN)
    assert not init_poll.poll(0)
    assert list(Path('/sys/fs/fuse/connections').iterdir())
    Path('/work/coordinator-owned-handles.json').write_text(json.dumps({
        'coordinator': os.getpid(), 'arbiter': arbiter.pid,
        'launcher': launcher.pid, 'init': int(init_pid)}))
    # Observer pins the identities while all actors are held alive. No recipe
    # output or numeric PID from an untrusted process supplies this authority.
    deadline = time.monotonic() + 5
    while not Path('/work/coordinator-observed').exists():
        assert time.monotonic() < deadline
        time.sleep(.01)
    raise AssertionError('observer must terminate coordinator at the owned gate')

finally:
    os.close(parent_fd)
    if launcher is not None and launcher.poll() is None:
        signal.pidfd_send_signal(launcher_fd, signal.SIGKILL)
        launcher.wait(timeout=5)
    if init_fd is not None:
        os.close(init_fd)
    if launcher_fd is not None:
        os.close(launcher_fd)
    if arbiter.poll() is None:
        signal.pidfd_send_signal(arbiter_fd, signal.SIGKILL)
        arbiter.wait(timeout=5)
    os.close(arbiter_fd)
