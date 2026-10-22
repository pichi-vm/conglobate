#!/usr/bin/env python3
"""Atomic publication of a locked lease, using owned host tmpfs directories."""
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
from records import recover_exclusive


def line(stream):
    value = bytearray()
    while not value.endswith(b'\n'):
        assert select.select([stream], [], [], 5)[0]
        byte = os.read(stream.fileno(), 1)
        assert byte and len(value) < 128
        value.extend(byte)
    return value.decode().strip()


def busy(path):
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        return False
    finally:
        os.close(fd)


def main():
    binary = Path(__file__).resolve().parents[2] / 'entry/helper/target/debug/commit-arbiter'
    outcomes = []
    with tempfile.TemporaryDirectory(prefix='conglobate-lease-bootstrap-', dir='/dev/shm') as tmp:
        root = Path(tmp)
        for mode in ('death-private', 'publish', 'collision'):
            directory = root / mode
            directory.mkdir(mode=0o700)
            final = directory / 'recovery.lease'
            if mode == 'collision':
                final.write_bytes(b'owned prior attempt')
                previous = final.stat().st_ino
            child = subprocess.Popen([str(binary), str(directory), '--lease-gate'],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            pidfd = os.pidfd_open(child.pid)
            try:
                assert line(child.stdout) == 'LEASE_LOCKED_PRIVATE'
                private = directory / 'recovery.lease.new'
                inode = private.stat().st_ino
                assert busy(private)
                if mode != 'collision':
                    assert not final.exists()
                    assert recover_exclusive(directory) == 'Unknown'
                if mode == 'death-private':
                    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                    child.wait(timeout=3)
                    assert not final.exists() and not busy(private)
                    assert recover_exclusive(directory) == 'Unknown'
                    outcomes.append('death-before-lease-publication-remains-unknown')
                else:
                    child.stdin.write(b'publish\n'); child.stdin.flush()
                    if mode == 'collision':
                        child.wait(timeout=3)
                        assert child.returncode != 0
                        assert final.stat().st_ino == previous
                        assert final.read_bytes() == b'owned prior attempt'
                        assert private.stat().st_ino == inode and not busy(private)
                        outcomes.append('existing-lease-preserved-no-replace')
                    else:
                        assert line(child.stdout) == 'LEASE_PUBLISHED_LOCKED'
                        assert final.stat().st_ino == inode and not private.exists()
                        assert busy(final)
                        assert recover_exclusive(directory) == 'Busy'
                        outcomes.append('published-inode-already-exclusively-locked')
                        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                        child.wait(timeout=3)
                        assert not busy(final)
                        assert final.stat().st_ino == inode
                        outcomes.append('publisher-death-releases-stable-published-lease')
            finally:
                if child.poll() is None:
                    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                    child.wait(timeout=3)
                os.close(pidfd)
                _, err = child.communicate(timeout=3)
                if mode != 'collision':
                    assert not err, err
                else:
                    assert b'EEXIST' in err, err
    print(json.dumps({'status': 'pass', 'cases': outcomes, 'count': len(outcomes)}, indent=2))


if __name__ == '__main__':
    signal.alarm(30)
    main()
