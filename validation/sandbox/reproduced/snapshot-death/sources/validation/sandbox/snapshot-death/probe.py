#!/usr/bin/python3
"""Kill only the fixture creator at known stopped ownership transitions."""
import os
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
    subprocess.run(['unshare', '--mount', '/snapshot-death', '--recover'],
                   env=env, check=True, timeout=5)
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
