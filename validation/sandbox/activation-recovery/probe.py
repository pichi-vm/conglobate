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
# Real writable snapshots retain a loop-backed persistent COW and a separate
# zero origin. Simulate the post-creator-death state without removing anything
# from outside this disposable guest. Production Scratch::new does recovery.
for held in (False, True):
    lease = 'cowheld' if held else 'cowclosed'
    directory = root / f'conglobate-{lease}'
    directory.mkdir(mode=0o700)
    name = f'conglobate-0-{lease}-0'
    origin = f'fixture-origin-{lease}'
    cow = Path('/work') / f'{lease}.cow'
    with cow.open('wb') as stream:
        stream.truncate(4 * 1024 * 1024)
    loop = subprocess.check_output(['losetup', '--find', '--show', str(cow)], text=True).strip()
    dm('create', origin, '--noudevsync', '--table', '0 16384 zero')
    minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', origin))
    origin_path = f'/dev/dm-{minor}'
    dm('create', name, '--noudevsync', '--table', f'0 16384 snapshot {origin_path} {loop} P 8')
    minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', name))
    fd = os.open(f'/dev/dm-{minor}', os.O_RDWR)
    payload = (b'bounded COW recovery canary ' * 200)[:4096]
    assert len(payload) == 4096
    assert os.pwrite(fd, payload, 0) == 4096
    os.fsync(fd)
    assert os.pread(fd, 4096, 0) == payload
    check = os.open(origin_path, os.O_RDONLY)
    assert os.pread(check, 4096, 0) == bytes(4096)
    os.close(check)
    # Dependencies now own their lifetimes. Recovery must remove only the dead
    # snapshot lease; kernel deferred removal/autoclear releases its resources.
    dm('remove', origin, '--noudevsync', '--deferred')
    subprocess.run(['losetup', '--detach', loop], check=True, timeout=5)
    if not held:
        os.close(fd)
    recover()
    assert not directory.exists()
    if held:
        assert exists(name) and exists(origin)
        assert os.pread(fd, 4096, 0) == payload
        assert os.pwrite(fd, payload, 4096) == 4096
        os.fsync(fd)
        assert os.pread(fd, 4096, 4096) == payload
        print('COW_RECOVERY_HELD_WRITABLE_HANDLE_PASS', flush=True)
        os.close(fd)
    gone(name)
    gone(origin)
    deadline = time.monotonic() + 3
    while subprocess.check_output(['losetup', '-j', str(cow)], text=True).strip() and time.monotonic() < deadline:
        time.sleep(.01)
    assert not subprocess.check_output(['losetup', '-j', str(cow)], text=True).strip()
    # Reassemble the persistent snapshot to prove COW exception data survived
    # teardown; this is a fresh fixture mapping, not resurrection by recovery.
    loop = subprocess.check_output(['losetup', '--find', '--show', str(cow)], text=True).strip()
    dm('create', origin, '--noudevsync', '--table', '0 16384 zero')
    minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', origin))
    dm('create', name, '--noudevsync', '--table', f'0 16384 snapshot /dev/dm-{minor} {loop} P 8')
    minor = int(dm('info', '-c', '--noheadings', '-o', 'minor', name))
    fd = os.open(f'/dev/dm-{minor}', os.O_RDONLY)
    assert os.pread(fd, 4096, 0) == payload
    assert os.pread(fd, 4096, 4096) == (payload if held else bytes(4096))
    os.close(fd)
    dm('remove', name, '--noudevsync')
    dm('remove', origin, '--noudevsync')
    subprocess.run(['losetup', '--detach', loop], check=True, timeout=5)
    print('COW_RECOVERY_DEPENDENCIES_AND_PERSISTENCE_PASS', lease, flush=True)
assert not subprocess.check_output(['losetup', '-a'], text=True).strip()
# Fixture-owned negative controls require explicit cleanup, not broad recovery.
for lease in leases[1:]:
    dm('remove', f'conglobate-0-{lease}-0', '--noudevsync')
assert dm('ls') == 'No devices found'
print('ACTIVATION_RECOVERY_MATRIX_PASS', flush=True)
