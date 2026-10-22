#!/usr/bin/python3
"""Cancel only the owned trusted resolver setup in a disposable chain build."""
import os
from pathlib import Path
import signal
import subprocess
import time

stage = Path('/resolver-stage').read_text().strip()
assert stage in ('before-bind', 'after-bind', 'after-readonly')
source = Path('/work/resolver')
target = Path('/work/view/run/image-resolv')


def identity(path):
    stat = path.stat()
    return stat.st_dev, stat.st_ino, stat.st_mode, stat.st_uid, stat.st_gid, path.read_bytes()


expected = identity(source), identity(target)
mounts = Path('/proc/self/mountinfo').read_text()
environment = dict(os.environ, RESOLVER_TEST_GATE=stage)
child = subprocess.Popen(['unshare', '--mount', '--pid', '--ipc', '--uts', '--net',
                          '--fork', '/bin/bash', '/command-launch'], env=environment,
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
ready = Path('/work/resolver-ready')
deadline = time.monotonic() + 8
while not ready.exists():
    if child.poll() is not None:
        output, _ = child.communicate(timeout=1)
        raise AssertionError(f'setup exited before its gate: {output.decode()}')
    assert time.monotonic() < deadline, 'resolver gate timeout'
    time.sleep(.01)
observed, pid = ready.read_text().split()
assert observed == stage
pid = int(pid)
pidfd = os.pidfd_open(pid)
try:
    namespace = Path(f'/proc/{pid}/ns/mnt').readlink()
    assert namespace != Path('/proc/self/ns/mnt').readlink()
    assert (identity(source), identity(target)) == expected
    assert Path('/proc/self/mountinfo').read_text() == mounts
    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
    output, _ = child.communicate(timeout=5)
    assert child.returncode != 0
    assert not Path(f'/proc/{pid}').exists()
    assert (identity(source), identity(target)) == expected
    assert Path('/proc/self/mountinfo').read_text() == mounts
    for path in Path('/proc').glob('[0-9]*/ns/mnt'):
        try:
            observed_namespace = path.readlink()
        except (FileNotFoundError, PermissionError):
            continue
        assert observed_namespace != namespace, path
    assert b'RESOLVER_FUSE_DESCRIPTOR_INSTALL_PASS' not in output
    assert b'CHAIN_RESTRICTED_COMMAND_PASS' not in output
    print(output.decode(), end='', flush=True)
    print('RESOLVER_CANCEL_STAGE_PASS', stage, flush=True)
finally:
    os.close(pidfd)
# Fail the actual worker-status gate before sealing/publication.
raise SystemExit(125)
