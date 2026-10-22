#!/usr/bin/python3
"""Owned disposable-guest process ordering probe; no recipe or external target."""
import ctypes
import fcntl
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

NEWPID = 0x20000000
libc = ctypes.CDLL(None, use_errno=True)


def syscall(name, *args):
    assert getattr(libc, name)(*args) == 0, (name, ctypes.get_errno())


def line(stream):
    data = bytearray()
    while not data.endswith(b'\n'):
        assert select.select([stream], [], [], 5)[0], 'readiness timeout'
        byte = os.read(stream.fileno(), 1)
        assert byte and len(data) < 128
        data.extend(byte)
    return data.decode().strip()


def readable(fd, timeout=0):
    return bool(select.select([fd], [], [], timeout)[0])


def acquire(root):
    fd = os.open(root / 'lease', os.O_RDWR | os.O_NOFOLLOW)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return fd
    except BlockingIOError:
        os.close(fd)
        return None


def run():
    root = Path('/work/recovery-lifetime')
    root.mkdir(mode=0o700)
    (root / 'lease').touch(mode=0o600)
    (root / 'sentinel').write_text('owned staging')
    original = os.open('/proc/self/ns/pid_for_children', os.O_RDONLY)
    init = outside = None
    initfd = nsfd = lease = None
    try:
        syscall('unshare', NEWPID)
        try:
            init = subprocess.Popen([sys.executable, __file__, '--init', str(root)],
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        finally:
            syscall('setns', original, NEWPID)
        initfd = os.pidfd_open(init.pid)
        assert line(init.stdout) == 'INIT_LEASE_HELD'
        assert acquire(root) is None
        nsfd = os.open(f'/proc/{init.pid}/ns/pid', os.O_RDONLY)
        # This trusted outside parent stays outside the namespace, while only
        # its next child enters it. It deliberately defers reaping that child.
        outside = subprocess.Popen([sys.executable, __file__, '--outside', str(nsfd)],
                                   pass_fds=(nsfd,), stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        assert line(outside.stdout) == 'OUTSIDE_CHILD_ZOMBIE'
        signal.pidfd_send_signal(initfd, signal.SIGKILL)
        deadline = time.monotonic() + 5
        while lease is None:
            assert time.monotonic() < deadline, 'init did not release lease'
            lease = acquire(root)
            if lease is None:
                time.sleep(.01)
        # exit_files released the init's FD, but namespace teardown is waiting
        # for the deliberately unreaped, non-writing owned child.
        assert not readable(initfd, .1)
        assert outside.poll() is None
        assert (root / 'sentinel').read_text() == 'owned staging'
        print('RECOVERY_LEASE_AVAILABLE_NAMESPACE_INIT_NOT_TERMINAL', flush=True)
        print('RECOVERY_PIDFD_BARRIER_DEFERS_CLEANUP', flush=True)
        outside.stdin.write(b'reap\n'); outside.stdin.flush()
        out, err = outside.communicate(timeout=5)
        assert outside.returncode == 0 and not out and not err
        assert readable(initfd, 5)
        init.wait(timeout=5)
        assert init.returncode == -signal.SIGKILL
        assert (root / 'sentinel').read_text() == 'owned staging'
        print('RECOVERY_NAMESPACE_REAP_BARRIER_PASS', flush=True)
    finally:
        # Reap outside child before waiting for namespace init, including when
        # an assertion fails; all processes belong solely to this fixture.
        if outside is not None and outside.poll() is None:
            try:
                outside.communicate(b'reap\n', timeout=5)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                outside.kill(); outside.wait(timeout=3)
        if init is not None:
            if init.poll() is None:
                if initfd is not None:
                    signal.pidfd_send_signal(initfd, signal.SIGKILL)
                else:
                    init.kill()
            init.communicate(timeout=5)
        for fd in (lease, nsfd, initfd, original):
            if fd is not None:
                os.close(fd)
    print('RECOVERY_LIFETIME_MATRIX_PASS', flush=True)


if __name__ == '__main__':
    signal.alarm(25)
    if len(sys.argv) == 3 and sys.argv[1] == '--init':
        assert os.getpid() == 1
        fd = acquire(Path(sys.argv[2]))
        assert fd is not None
        print('INIT_LEASE_HELD', flush=True)
        while True:
            signal.pause()
    elif len(sys.argv) == 3 and sys.argv[1] == '--outside':
        syscall('setns', int(sys.argv[2]), NEWPID)
        child = subprocess.Popen([sys.executable, '-c', 'pass'])
        childfd = os.pidfd_open(child.pid)
        try:
            assert readable(childfd, 5)
            # Do not call poll/wait here: it must remain a zombie until release.
            print('OUTSIDE_CHILD_ZOMBIE', flush=True)
            assert select.select([sys.stdin], [], [], 10)[0]
            assert sys.stdin.readline() == 'reap\n'
            assert child.wait(timeout=3) == 0
        finally:
            child.wait(timeout=3)
            os.close(childfd)
    else:
        run()
