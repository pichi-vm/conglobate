#!/usr/bin/env python3
"""Bounded cooperative staging-lease probe, using only owned tmpfs files."""
import fcntl
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile


def line(stream):
    value = bytearray()
    while not value.endswith(b'\n'):
        assert select.select([stream], [], [], 5)[0], 'child readiness timeout'
        byte = os.read(stream.fileno(), 1)
        assert byte, 'unexpected child exit'
        value.extend(byte)
        assert len(value) <= 128, 'oversized readiness line'
    return value.decode().strip()


def acquire(path):
    fd = os.open(path, os.O_RDWR | os.O_CLOEXEC)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    return fd


def child(mode, arg):
    if mode == 'descendant':
        # Inherited lease is held until this owned process exits. No unlock:
        # LOCK_UN would release the shared open-file-description lock early.
        fd = int(arg)
        os.fstat(fd)
        print('descendant-ready', flush=True)
        assert sys.stdin.readline() == 'exit\n'
        os.close(fd)
    elif mode == 'owner':
        fd = acquire(arg)
        assert fd is not None
        descendant = subprocess.Popen(
            [sys.executable, __file__, 'descendant', str(fd)],
            pass_fds=(fd,), stdin=sys.stdin, stdout=sys.stdout)
        print('owner-exiting', flush=True)
        # Parent intentionally exits without waiting; the harness is a Linux
        # subreaper so it can reap the descendant, rather than leaving a zombie.
        os.close(fd)
    elif mode == 'recoverer':
        fd = acquire(arg)
        if fd is None:
            print('busy', flush=True)
            return
        print('exclusive', flush=True)
        assert sys.stdin.readline() == 'exit\n'
        os.close(fd)


def run():
    # Diagnostic-only process supervision; production Rust forbids unsafe.
    import ctypes
    libc = ctypes.CDLL(None, use_errno=True)
    assert libc.prctl(36, 1, 0, 0, 0) == 0  # PR_SET_CHILD_SUBREAPER
    events = []
    with tempfile.TemporaryDirectory(prefix='conglobate-lease-', dir='/dev/shm') as tmp:
        root = Path(tmp)
        lease = root / 'lease'
        lease.touch(mode=0o600)
        stage = root / 'staging'
        stage.mkdir()
        (stage / 'sentinel').write_text('owned staging')
        original = lease.stat().st_ino
        fd = acquire(lease)
        assert fd is not None and acquire(lease) is None
        events.append('live-owner-blocks-independent-acquisition')
        alias = os.dup(fd)
        fcntl.flock(alias, fcntl.LOCK_UN)
        competitor = acquire(lease)
        assert competitor is not None
        os.close(competitor)
        os.close(alias)
        os.close(fd)
        events.append('negative-shared-description-unlock-releases-live-holder')
        # Separate owned negative: unlink/recreate splits the lock domain.
        split = root / 'split-lease'
        split.touch(mode=0o600)
        old = acquire(split)
        split.unlink()
        split.touch(mode=0o600)
        new = acquire(split)
        assert old is not None and new is not None
        assert os.fstat(old).st_ino != os.fstat(new).st_ino
        os.close(old); os.close(new)
        events.append('negative-recreated-lease-allows-two-lock-domains')
        owner = subprocess.Popen([sys.executable, __file__, 'owner', str(lease)],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 text=True, bufsize=1)
        assert {line(owner.stdout), line(owner.stdout)} == {'owner-exiting', 'descendant-ready'}
        owner.wait(timeout=5)
        assert owner.returncode == 0
        assert acquire(lease) is None
        assert (stage / 'sentinel').read_text() == 'owned staging'
        events.append('exited-owner-surviving-descendant-retains-lease')
        owner.stdin.write('exit\n'); owner.stdin.flush()
        # Pipe EOF proves inherited stdout closed, then reap the owned adopted
        # descendant. Timer bounds the whole harness, including waitpid.
        assert select.select([owner.stdout], [], [], 5)[0]
        assert owner.stdout.read() == ''
        _, status = os.waitpid(-1, 0)
        assert os.waitstatus_to_exitcode(status) == 0
        fd = acquire(lease)
        assert fd is not None
        os.close(fd)
        events.append('all-holders-exited-lease-becomes-acquirable')
        winner = subprocess.Popen([sys.executable, __file__, 'recoverer', str(lease)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        assert line(winner.stdout) == 'exclusive'
        loser = subprocess.run([sys.executable, __file__, 'recoverer', str(lease)],
                               capture_output=True, text=True, timeout=5, check=True)
        assert loser.stdout.strip() == 'busy'
        assert lease.stat().st_ino == original
        assert (stage / 'sentinel').read_text() == 'owned staging'
        events.append('independent-recoverer-excluded-on-stable-lease-inode')
        winner.stdin.write('exit\n'); winner.stdin.flush()
        winner.wait(timeout=5)
        assert winner.returncode == 0
        # No public artifact is created or removed. GC is deliberately not
        # implemented: a lock alone does not prove writer admission was sound.
    print(json.dumps({'cases': events, 'count': len(events), 'status': 'pass'}, indent=2))


if __name__ == '__main__':
    import signal
    signal.alarm(25)
    if len(sys.argv) > 1:
        child(sys.argv[1], sys.argv[2])
    else:
        run()
