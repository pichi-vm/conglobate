#!/usr/bin/python3
"""Cancel trusted setup before any image command is launched."""
import os
from pathlib import Path
import signal
import subprocess
import time

image = Path('/work/image/etc')
image.mkdir(parents=True)
source = Path('/work/resolver')
target = image / 'resolv.conf'
source.write_bytes(b'nameserver 192.0.2.53\n')
target.write_bytes(b'# image-owned resolver\nnameserver 127.0.0.53\n')

def identity(path):
    stat = path.stat()
    return (stat.st_dev, stat.st_ino, stat.st_uid, stat.st_gid, stat.st_mode, path.read_bytes())

expected = (identity(source), identity(target))
mounts = Path('/proc/self/mountinfo').read_text()
for stage in ['before-bind', 'after-bind', 'after-readonly']:
    ready = Path('/work/ready')
    ready.unlink(missing_ok=True)
    child = subprocess.Popen(['unshare', '--mount', '--pid', '--fork', '/bin/bash', '/launch', stage],
                             stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    deadline = time.monotonic() + 5
    while not ready.exists() or not ready.read_text().strip():
        assert child.poll() is None
        assert time.monotonic() < deadline, 'setup gate timeout'
        time.sleep(.01)
    pid = int(ready.read_text())
    pidfd = os.pidfd_open(pid)
    # The trusted init is held at a bounded gate. The parent never observes
    # injected bytes; the source and image inodes remain untouched even live.
    assert (identity(source), identity(target)) == expected
    assert Path('/proc/self/mountinfo').read_text() == mounts
    ns = Path(f'/proc/{pid}/ns/mnt').readlink()
    assert ns != Path('/proc/self/ns/mnt').readlink()
    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
    output, _ = child.communicate(timeout=5)
    os.close(pidfd)
    assert child.returncode != 0
    assert not Path(f'/proc/{pid}').exists(), 'namespace init remains'
    assert (identity(source), identity(target)) == expected
    assert Path('/proc/self/mountinfo').read_text() == mounts
    # No process may retain the failed setup's mount namespace.
    holders = []
    for path in Path('/proc').glob('[0-9]*/ns/mnt'):
        try:
            if path.readlink() == ns:
                holders.append(str(path))
        except FileNotFoundError:
            pass
    assert not holders, holders
    if stage == 'after-readonly':
        assert b'Read-only file system' in output, output
    print('RESOLVER_SETUP_CANCELLATION_PASS',stage,flush=True)
print('RESOLVER_SETUP_FILES_AND_PARENT_MOUNTS_PRESERVED',flush=True)
