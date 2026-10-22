#!/usr/bin/python3
"""Cancel a bounded fixture command through its pinned namespace-init pidfd."""
import os
from pathlib import Path
import signal
import subprocess
import time

child = subprocess.Popen(['unshare', '--mount', '--pid', '--ipc', '--uts', '--net',
                          '--fork', '/bin/bash', '/command-launch'],
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT)
deadline = time.monotonic() + 10
pidfile = Path('/work/command.pid')
while not pidfile.exists():
    assert child.poll() is None, 'launcher exited before handing off PID'
    assert time.monotonic() < deadline, 'PID handoff timeout'
    time.sleep(.01)
pidfd = os.pidfd_open(int(pidfile.read_text()))
# Trusted launch waits at a gate, so no untrusted code can exit/reuse that PID
# before the handle is pinned. Commands cannot reach the handoff directory.
Path('/work/command-go').touch()
paths = [Path('/work/view') / name for name in ('heartbeat', 'child-heartbeat')]
def sizes():
    return [p.stat().st_size if p.exists() else 0 for p in paths]
try:
    while min(sizes()) < 3:
        assert child.poll() is None, 'command finished before cancellation'
        assert time.monotonic() < deadline, 'writer startup timeout'
        time.sleep(.02)
    before = sizes()
    time.sleep(.1)
    assert all(a > b for a, b in zip(sizes(), before)), 'writers are not active'
    print('WORKER_OOM_TWO_WRITERS_LIVE', flush=True)
    daemon = os.pidfd_open(int(Path('/work/daemon.pid').read_text()))
    try:
        signal.pidfd_send_signal(daemon, signal.SIGSTOP)
        Path('/work/worker-stopped').touch()
        # Do not touch the FUSE mount while its worker is stopped.
        deadline = time.monotonic() + 15
        while not Path('/work/cancel-now').exists():
            assert time.monotonic() < deadline, 'supervisor queue observation timeout'
            time.sleep(.01)
        try:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
        except ProcessLookupError:
            pass  # FUSE disconnection may already have ended the command.
        Path('/work/clients-signaled').touch()
        while not Path('/work/abort-done').exists():
            assert time.monotonic() < deadline, 'supervisor abort timeout'
            time.sleep(.01)
        output, _ = child.communicate(timeout=10)
        assert child.returncode != 0, child.returncode
        print(output.decode(), end='', flush=True)
        print('WORKER_OOM_COMMAND_REAPED', flush=True)
    finally:
        os.close(daemon)
finally:
    os.close(pidfd)
# Return failure through worker status gate; no snapshot sealing/publication.
raise SystemExit(125)
