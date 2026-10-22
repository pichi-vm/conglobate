#!/usr/bin/python3
"""Bounded interrupted-activation states; no host device access."""
import fcntl
import os
from pathlib import Path
import subprocess
import time

root = Path('/work/leases')
root.mkdir(mode=0o700)
env = dict(os.environ, TMPDIR=str(root))

def dm(*args):
    return subprocess.check_output(['dmsetup', *args], text=True).strip()

def exists(name):
    return subprocess.run(['dmsetup', 'info', name], stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL).returncode == 0

def recover():
    subprocess.run(['/recovery-helper'], env=env, check=True, timeout=5)

def gone(name):
    deadline = time.monotonic() + 3
    while exists(name) and time.monotonic() < deadline:
        time.sleep(.01)
    assert not exists(name), name

# These represent persistent kernel states after a creator dies before setting
# deferred removal. They are constructed directly, not injected into a live
# Snapshot::create call. Both use the actual Scratch::new recovery entry point.
for index, active in enumerate([False, True]):
    lease = f'dead{index}'
    directory = root / f'conglobate-{lease}'
    directory.mkdir(mode=0o700)
    name = f'conglobate-0-{lease}-0'
    dm('create', name, '--noudevsync', *(['--table', '0 8 zero'] if active else ['--notable']))
    assert exists(name)
    recover()
    gone(name)
    assert not directory.exists()
    print('ACTIVATION_RECOVERY_PASS', 'active' if active else 'created', flush=True)

# A competing live lease, an unsafe directory mode, nonempty state and a symlink
# must not authorize recovery of similarly named devices.
leases = ['live', 'mode', 'nonempty', 'link', 'foreignuid']
for lease in leases:
    directory = root / f'conglobate-{lease}'
    if lease == 'link':
        target = root / 'link-target'
        target.mkdir(mode=0o700)
        directory.symlink_to(target)
    else:
        directory.mkdir(mode=0o700)
    if lease == 'foreignuid':
        os.chown(directory, 1234, 1234)
    if lease == 'mode':
        directory.chmod(0o755)
    if lease == 'nonempty':
        (directory / 'keep').write_text('untouched')
    dm('create', f'conglobate-0-{lease}-0', '--noudevsync', '--notable')
live = os.open(root / 'conglobate-live', os.O_RDONLY | os.O_DIRECTORY)
fcntl.flock(live, fcntl.LOCK_EX)
recover()
for lease in leases:
    assert exists(f'conglobate-0-{lease}-0'), lease
    assert (root / f'conglobate-{lease}').exists(), lease
assert (root / 'conglobate-nonempty/keep').read_text() == 'untouched'
print('ACTIVATION_RECOVERY_OTHER_LEASES_PRESERVED', flush=True)
# Once the live owner releases its lifetime lock, startup can recover it.
os.close(live)
recover()
gone('conglobate-0-live-0')
assert not (root / 'conglobate-live').exists()
print('ACTIVATION_RECOVERY_RELEASED_LEASE_PASS', flush=True)
# An already-deferred device belongs to kernel lifetime management. Recovery
# may collect its dead directory but must preserve an independent open reader.
directory = root / 'conglobate-deferred'
directory.mkdir(mode=0o700)
name = 'conglobate-0-deferred-0'
dm('create', name, '--noudevsync', '--table', '0 8 zero')
minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', name))
fd = os.open(f'/dev/dm-{minor}', os.O_RDONLY)
dm('remove', name, '--noudevsync', '--deferred')
recover()
assert not directory.exists()
assert exists(name)
assert os.pread(fd, 4096, 0) == bytes(4096)
os.close(fd)
gone(name)
print('ACTIVATION_RECOVERY_DEFERRED_READER_PASS', flush=True)
# Fixture-owned negative controls require explicit cleanup, not broad recovery.
for lease in leases[1:]:
    dm('remove', f'conglobate-0-{lease}-0', '--noudevsync')
assert dm('ls') == 'No devices found'
print('ACTIVATION_RECOVERY_MATRIX_PASS', flush=True)
