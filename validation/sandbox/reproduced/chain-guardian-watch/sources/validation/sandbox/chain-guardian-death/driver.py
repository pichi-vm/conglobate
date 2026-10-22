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
    print('SUPERVISOR_DEATH_TWO_WRITERS_LIVE', flush=True)
    Path('/work/writers-ready').touch()
    # Guardian will kill this process with the rest of the private cgroup.
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        assert child.poll() is None, 'command unexpectedly exited'
        Path('/work/writers-progress.tmp').write_text(' '.join(map(str,sizes())))
        Path('/work/writers-progress.tmp').replace('/work/writers-progress')
        time.sleep(.05)
    raise AssertionError('guardian did not terminate the fixture')
finally:
    os.close(pidfd)
# Return failure through worker status gate; no snapshot sealing/publication.
raise SystemExit(125)
