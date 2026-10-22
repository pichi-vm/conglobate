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
assert mode in ('cancel', 'success')
arbiter = subprocess.Popen(['/commit-arbiter', str(root), '--await-completion'])
arbiter_fd = os.pidfd_open(arbiter.pid)
launcher = None
launcher_fd = None

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
    print('LIVE_CONTROL_SOCKET_READY_BEFORE_RECIPE_PASS', flush=True)
    launcher = subprocess.Popen(['python3', '/outer-launcher.py'])
    launcher_fd = os.pidfd_open(launcher.pid)
    deadline = time.monotonic() + 90
    while not Path('/work/admission-active').exists():
        assert launcher.poll() is None and time.monotonic() < deadline
        time.sleep(.01)
    assert rpc('status') == 'Building'
    print('LIVE_CONTROL_SOCKET_STILL_RESPONSIVE_PASS', flush=True)
    # This owned marker only schedules the experiment. It never authorizes commit.
    assert rpc('reserve') == 'Building'
    assert rpc('commit') == 'Building'
    assert not (root / 'result').exists()
    print('LIVE_BUILD_PREMATURE_COMMIT_REJECTED_PASS', flush=True)
    if mode == 'cancel':
        assert rpc('cancel') == 'Accepted'
        print('LIVE_BUILD_CANCELLATION_ACCEPTED_PASS', flush=True)
    Path('/work/admission-continue').touch()
    assert launcher.wait(timeout=100) == 0
    poller = select.poll()
    poller.register(launcher_fd, select.POLLIN)
    assert poller.poll(0)
    # The dedicated launcher reaps namespace init, whose final checks reject
    # live descendants. Publication remains outside that lifetime namespace.
    deadline = time.monotonic() + 5
    while True:
        dm = subprocess.check_output(['dmsetup', 'ls'], timeout=2).decode().strip()
        loops = subprocess.check_output(['losetup', '-a'], timeout=2).decode().strip()
        if dm == 'No devices found' and not loops:
            break
        assert time.monotonic() < deadline
        time.sleep(.02)
    Path('/work/published').rename(root / 'published')
    private, public = root / 'published', root / 'result'
    assert (private / 'inventory.cbor').is_file()
    before = hashes(private)
    print('BUILD_ADMISSION_COMPLETION_BARRIER_PASS', flush=True)
    if mode == 'cancel':
        assert rpc('prepare') == rpc('reserve') == rpc('commit') == 'Cancelled'
        assert not public.exists() and hashes(private) == before
        print('LIVE_CANCEL_PREVENTS_LATE_PUBLICATION_PASS', flush=True)
    else:
        assert rpc('prepare') == 'Prepared'
        assert rpc('reserve') == 'Reserved'
        assert rpc('commit') == 'Committed'
        assert not private.exists() and hashes(public) == before
        print('BUILD_ADMISSION_POST_BARRIER_COMMIT_PASS', flush=True)
finally:
    if launcher is not None and launcher.poll() is None:
        signal.pidfd_send_signal(launcher_fd, signal.SIGKILL)
        launcher.wait(timeout=5)
    if launcher_fd is not None:
        os.close(launcher_fd)
    if arbiter.poll() is None:
        signal.pidfd_send_signal(arbiter_fd, signal.SIGKILL)
        arbiter.wait(timeout=5)
    os.close(arbiter_fd)
