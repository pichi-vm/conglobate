#!/usr/bin/python3
"""Trusted diagnostic coordinator; no recipe logs authorize publication."""
import hashlib
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
assert mode == 'cancel-stop'
arbiter = subprocess.Popen(['/commit-arbiter', str(root), '--await-completion'])
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
    launcher = subprocess.Popen(['python3', '/outer-launcher.py'])
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
    assert rpc('cancel') == 'Accepted'
    print('LIVE_BUILD_CANCELLATION_ACCEPTED_PASS', flush=True)
    signal.pidfd_send_signal(launcher_fd, signal.SIGKILL)
    assert launcher.wait(timeout=5) == -signal.SIGKILL
    assert init_poll.poll(5000), 'namespace init has not finished teardown'
    print('CANCEL_ADMISSION_NAMESPACE_TERMINAL_PASS', flush=True)
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
    assert not Path('/work/published').exists()
    assert not (root / 'published').exists() and not (root / 'result').exists()
    assert rpc('prepare') == rpc('reserve') == rpc('commit') == 'Cancelled'
    print('CANCEL_ADMISSION_STORAGE_BASE_AND_NO_OUTPUT_PASS', flush=True)

finally:
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
