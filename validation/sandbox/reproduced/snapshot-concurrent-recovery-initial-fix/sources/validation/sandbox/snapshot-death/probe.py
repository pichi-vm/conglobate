#!/usr/bin/python3
"""Kill only the fixture creator at known stopped ownership transitions."""
import os
import fcntl
from pathlib import Path
import signal
import subprocess
import time

root = Path('/work/leases')
root.mkdir(mode=0o700)
env = dict(os.environ, TMPDIR=str(root))
def dm(*args):
    return subprocess.check_output(['dmsetup', *args], text=True, timeout=5).strip()

for stage in ('attached', 'created', 'loaded', 'resumed', 'opened', 'deferred'):
    Path('/work/reached').unlink(missing_ok=True)
    Path('/work/lease').unlink(missing_ok=True)
    dm('create', 'fixture-origin', '--noudevsync', '--table', '0 134217728 zero')
    minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', 'fixture-origin'))
    origin = f'/dev/dm-{minor}'
    holder = os.open(origin, os.O_RDONLY)
    dm('remove', 'fixture-origin', '--noudevsync', '--deferred')
    child = subprocess.Popen(['unshare', '--mount', '/snapshot-death', origin],
                             env=env | {'SNAPSHOT_STOP': stage})
    pidfd = os.pidfd_open(child.pid)
    try:
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            assert child.poll() is None, (stage, child.returncode)
            status = Path(f'/proc/{child.pid}/status').read_text()
            if Path('/work/reached').exists() and '\nState:\tT' in status:
                break
            time.sleep(.01)
        else:
            raise AssertionError(f'creator did not stop: {stage}')
        assert Path('/work/reached').read_text() == stage
        lease = Path(Path('/work/lease').read_text())
        assert lease.parent == root
        # Creator's tmpfs is private: the underlying empty lease is visible here.
        assert lease.is_dir() and not list(lease.iterdir())
        print('SNAPSHOT_CREATOR_STOPPED', stage, dm('ls'), flush=True)
        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
        assert child.wait(timeout=5) == -signal.SIGKILL
    finally:
        if child.poll() is None:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            child.wait(timeout=5)
        os.close(pidfd)
        os.close(holder)
    # Hold the actual creation lock so both contenders demonstrably wait on
    # flock before either can inspect/remove the dead lease.
    lock = os.open(root / '.conglobate-scratch.lock', os.O_RDWR)
    fcntl.flock(lock, fcntl.LOCK_EX)
    contenders = []
    try:
        for _ in range(2):
            process = subprocess.Popen(['unshare', '--mount', '/snapshot-death', '--recover-gated'], env=env)
            handle = os.pidfd_open(process.pid)
            contenders.append((process, handle))
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert all(process.poll() is None for process, _ in contenders)
            if all('\nState:\tT' in Path(f'/proc/{process.pid}/status').read_text()
                   for process, _ in contenders):
                break
            time.sleep(.01)
        else:
            raise AssertionError('recovery contenders did not stop')
        for _, handle in contenders:
            signal.pidfd_send_signal(handle, signal.SIGCONT)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert all(process.poll() is None for process, _ in contenders)
            # This fixture is explicitly x86_64: __NR_flock is 73. Confirm each
            # process is blocked in LOCK_EX (2), not merely still alive.
            calls = [Path(f'/proc/{process.pid}/syscall').read_text().split()
                     for process, _ in contenders]
            if all(len(call) >= 3 and call[0] == '73' and int(call[2], 16) == 2 for call in calls):
                break
            time.sleep(.01)
        else:
            raise AssertionError(('recovery contenders did not block in flock', calls))
        # Confirm the blocked FDs refer to the exact lock held by this parent.
        expected = os.fstat(lock)
        for (process, _), call in zip(contenders, calls):
            actual = os.stat(f'/proc/{process.pid}/fd/{int(call[1], 16)}')
            assert (actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino)
        assert lease.is_dir()
        print('SNAPSHOT_RECOVERY_CONTENDERS_BLOCKED', stage, flush=True)
        fcntl.flock(lock, fcntl.LOCK_UN)
        for process, _ in contenders:
            assert process.wait(timeout=5) == 0
    finally:
        os.close(lock)
        for process, handle in contenders:
            if process.poll() is None:
                signal.pidfd_send_signal(handle, signal.SIGKILL)
                process.wait(timeout=5)
            os.close(handle)
    print('SNAPSHOT_CONCURRENT_RECOVERY_PASS', stage, flush=True)
    assert not lease.exists(), stage
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        devices = dm('ls')
        loops = subprocess.check_output(['losetup', '-a'], text=True, timeout=5).strip()
        if devices == 'No devices found' and not loops:
            break
        time.sleep(.01)
    assert devices == 'No devices found' and not loops, (stage, devices, loops)
    print('SNAPSHOT_CREATOR_DEATH_RECOVERY_PASS', stage, flush=True)
print('SNAPSHOT_CREATOR_DEATH_MATRIX_PASS', flush=True)
