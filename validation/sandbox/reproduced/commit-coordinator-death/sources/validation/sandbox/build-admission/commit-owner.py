#!/usr/bin/python3
"""Owned coordinator death at publication boundaries using a real build artifact."""
import hashlib
import os
from pathlib import Path
import select
import shutil
import signal
import socket
import subprocess
import sys
import time

def terminal(fd, timeout=0):
    poller = select.poll()
    poller.register(fd, select.POLLIN)
    return bool(poller.poll(timeout))

if len(sys.argv) == 3 and sys.argv[1] == '--owner':
    root = Path(sys.argv[2])
    parent = os.pidfd_open(os.getpid())
    child = subprocess.Popen(['/owned-exec', '/commit-arbiter', str(root), '--await-completion'], stdin=parent)
    os.close(parent)
    try:
        deadline = time.monotonic() + 5
        while not (root / 'control.sock').exists():
            assert child.poll() is None and time.monotonic() < deadline
            time.sleep(.01)
        (root / 'arbiter-pid.new').write_text(str(child.pid))
        (root / 'arbiter-pid.new').rename(root / 'arbiter-pid')
        child.wait(timeout=30)
        raise AssertionError('observer must kill this owner at the selected gate')
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=5)
    raise SystemExit(1)

assert int(Path('/work/owner-status').read_text()) == 0
source = Path('/work/published')
assert (source / 'inventory.cbor').is_file()

def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}

expected = hashes(source)
for stage in ('reserved', 'visible-unacknowledged'):
    root = Path('/work/commit-owner-' + stage)
    root.mkdir(mode=0o700)
    shutil.copytree(source, root / 'published')
    owner = subprocess.Popen(['/usr/bin/python3', '/commit-gate.py', '--owner', str(root)])
    owner_fd = os.pidfd_open(owner.pid)
    arbiter_fd = None
    pending = None

    def connect(operation):
        stream = socket.socket(socket.AF_UNIX)
        stream.settimeout(3)
        stream.connect(str(root / 'control.sock'))
        stream.sendall(operation.encode() + b'\n')
        return stream

    def rpc(operation):
        with connect(operation) as stream:
            reply = stream.makefile('rb').readline(64)
            assert reply.endswith(b'\n')
            return reply.decode().strip()

    try:
        deadline = time.monotonic() + 5
        while not (root / 'arbiter-pid').exists():
            assert owner.poll() is None and time.monotonic() < deadline
            time.sleep(.01)
        pid = int((root / 'arbiter-pid').read_text())
        arbiter_fd = os.pidfd_open(pid)
        assert int(Path(f'/proc/{pid}/status').read_text().split('PPid:')[1].splitlines()[0]) == owner.pid
        assert not terminal(owner_fd) and not terminal(arbiter_fd)
        assert rpc('status') == 'Building'
        # Real build, namespace completion and DM/loop checks precede this script.
        assert rpc('prepare') == 'Prepared'
        assert rpc('reserve') == 'Reserved'
        assert not (root / 'result').exists()
        if stage == 'visible-unacknowledged':
            pending = connect('commit-no-reply')
            deadline = time.monotonic() + 3
            while rpc('status') != 'Committed':
                assert time.monotonic() < deadline
                time.sleep(.01)
            pending.settimeout(.1)
            try:
                pending.recv(1)
            except TimeoutError:
                pass
            else:
                raise AssertionError('commit reply unexpectedly received')
            assert hashes(root / 'result') == expected
            identity = (root / 'result').stat().st_ino
        signal.pidfd_send_signal(owner_fd, signal.SIGKILL)
        assert owner.wait(timeout=5) == -signal.SIGKILL
        assert terminal(arbiter_fd, 5000), 'arbiter survived coordinator death'
        if stage == 'reserved':
            assert not (root / 'result').exists()
            assert hashes(root / 'published') == expected
        else:
            pending.settimeout(3)
            assert pending.recv(64) == b''
            assert not (root / 'published').exists()
            assert (root / 'result').stat().st_ino == identity
            assert hashes(root / 'result') == expected
        assert hashes(source) == expected
        print('COMMIT_COORDINATOR_DEATH_PASS ' + stage, flush=True)
    finally:
        if pending is not None:
            pending.close()
        if not terminal(owner_fd):
            signal.pidfd_send_signal(owner_fd, signal.SIGKILL)
        owner.wait(timeout=5)
        if arbiter_fd is not None:
            if not terminal(arbiter_fd):
                signal.pidfd_send_signal(arbiter_fd, signal.SIGKILL)
            os.close(arbiter_fd)
        os.close(owner_fd)
print('COMMIT_COORDINATOR_DEATH_MATRIX_PASS', flush=True)
