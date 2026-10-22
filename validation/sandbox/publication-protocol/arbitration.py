#!/usr/bin/python3
"""Owned-artifact arbitration tests; private control socket, not recipe logs."""
import concurrent.futures
import hashlib
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import threading
import time

assert int(Path('/work/owner-status').read_text()) == 0
source = Path('/work/published')
assert (source / 'inventory.cbor').is_file()


def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}


expected = hashes(source)
cases = ['cancel-first', 'reserve-first', 'collision', 'death-reserved', 'death-unacknowledged']
cases += ['race-' + str(i) for i in range(8)]
for name in cases:
    root = Path('/work/arbitrate-' + name)
    root.mkdir(mode=0o700)
    shutil.copytree(source, root / 'published')
    child = subprocess.Popen(['/commit-arbiter', str(root)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    pidfd = os.pidfd_open(child.pid)

    def connect(operation):
        stream = socket.socket(socket.AF_UNIX)
        stream.settimeout(3)
        stream.connect(str(root / 'control.sock'))
        stream.sendall(operation.encode() + b'\n')
        return stream

    def rpc(operation):
        with connect(operation) as stream:
            reply = stream.makefile('rb').readline(64)
            assert reply.endswith(b'\n'), (name, operation, reply)
            return reply.decode().strip()

    def kill_owned():
        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
        child.wait(timeout=3)
        assert child.returncode == -signal.SIGKILL

    try:
        deadline = time.monotonic() + 5
        while not (root / 'control.sock').exists():
            assert child.poll() is None and time.monotonic() < deadline
            time.sleep(.01)
        assert rpc('status') == 'Prepared'
        if name == 'death-reserved':
            assert rpc('reserve') == 'Reserved'
            kill_owned()
            assert not (root / 'result').exists()
            assert hashes(root / 'published') == expected
        elif name == 'death-unacknowledged':
            assert rpc('reserve') == 'Reserved'
            with connect('commit-no-reply') as pending:
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
                    raise AssertionError('commit unexpectedly acknowledged')
                kill_owned()
                pending.settimeout(3)
                assert pending.recv(64) == b''
            assert hashes(root / 'result') == expected
            assert not (root / 'published').exists()
        elif name == 'collision':
            assert rpc('reserve') == 'Reserved'
            (root / 'result').mkdir()
            (root / 'result/sentinel').write_bytes(b'owned foreign destination')
            identity = (root / 'result').stat().st_ino
            assert rpc('commit') == 'Collision'
            assert rpc('cancel') == 'Collision'
            assert (root / 'result').stat().st_ino == identity
            assert hashes(root / 'result') == {'sentinel': hashlib.sha256(b'owned foreign destination').hexdigest()}
            assert hashes(root / 'published') == expected
        else:
            if name == 'cancel-first':
                cancellation, reservation = rpc('cancel'), rpc('reserve')
            elif name == 'reserve-first':
                reservation, cancellation = rpc('reserve'), rpc('cancel')
            else:
                barrier = threading.Barrier(3)

                def race(operation):
                    barrier.wait(timeout=3)
                    return rpc(operation)

                with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                    cancel = pool.submit(race, 'cancel')
                    reserve = pool.submit(race, 'reserve')
                    barrier.wait(timeout=3)
                    cancellation, reservation = cancel.result(timeout=3), reserve.result(timeout=3)
            assert (cancellation, reservation) in [('Accepted', 'Cancelled'), ('Pending', 'Reserved')]
            if cancellation == 'Accepted':
                assert rpc('commit') == 'Cancelled'
                assert not (root / 'result').exists()
                assert hashes(root / 'published') == expected
            else:
                assert rpc('commit') == 'Committed'
                assert rpc('cancel') == 'Committed'
                assert hashes(root / 'result') == expected
                assert not (root / 'published').exists()
            print('ARBITRATION_OUTCOME', name, cancellation, reservation, flush=True)
        print('PUBLICATION_ARBITRATION_CASE_PASS', name, flush=True)
    finally:
        if child.poll() is None:
            kill_owned()
        os.close(pidfd)
        out, error = child.communicate(timeout=3)
        assert not out and not error, (name, out, error)
    shutil.rmtree(root)
assert hashes(source) == expected
print('PUBLICATION_ARBITRATION_MATRIX_PASS', flush=True)
