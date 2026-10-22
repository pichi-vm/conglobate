#!/usr/bin/python3
"""Pin the benign fixture child from outside its PID namespace and verify exit."""
import os
from pathlib import Path
import select
import subprocess
import time
launcher = subprocess.Popen(['python3', '/outer-launcher.py'])
fd = None
try:
    deadline = time.monotonic() + 100
    while not Path('/work/late-child-local').exists():
        assert launcher.poll() is None, 'launcher failed before test child'
        assert time.monotonic() < deadline
        time.sleep(.02)
    local = int(Path('/work/late-child-local').read_text())
    init, = Path(f'/proc/{launcher.pid}/task/{launcher.pid}/children').read_text().split()
    assert Path(f'/proc/{init}/status').read_text().split('NSpid:')[1].splitlines()[0].split() == [init, '1']
    candidates = []
    for pid in Path(f'/proc/{init}/task/{init}/children').read_text().split():
        status = Path(f'/proc/{pid}/status').read_text()
        if status.split('NSpid:')[1].splitlines()[0].split() == [pid, str(local)]:
            candidates.append(int(pid))
    child, = candidates
    fd = os.pidfd_open(child)
    poller = select.poll()
    poller.register(fd, select.POLLIN)
    assert not poller.poll(0)
    Path('/work/late-child-pinned').touch()
    result = launcher.wait(timeout=10)
    assert result == 1, result
    assert poller.poll(3000), 'fixture child survived namespace init failure'
    print('LATE_DESCENDANT_KERNEL_TERMINATION_CONFIRMED', flush=True)
    raise SystemExit(result)
finally:
    if fd is not None:
        os.close(fd)
    if launcher.poll() is None:
        launcher.kill()
        launcher.wait(timeout=5)
