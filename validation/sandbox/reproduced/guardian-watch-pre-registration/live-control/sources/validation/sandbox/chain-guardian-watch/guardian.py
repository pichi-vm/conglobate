#!/usr/bin/python3
"""Give the trusted supervisor monitor a pinned handle to this guardian."""
import os
from pathlib import Path
import subprocess

pidfd = os.pidfd_open(os.getpid())
try:
    if Path('/guardian-watch-stage').read_text().strip() == 'before-watch-registration':
        child = subprocess.Popen(['/bin/bash', '-c',
            'kill -STOP $$; exec python3 /monitor.py "$1"', 'monitor', str(pidfd)],
            pass_fds=(pidfd,))
        pid, status = os.waitpid(child.pid, os.WUNTRACED)
        assert pid == child.pid and os.WIFSTOPPED(status)
        Path('/work/monitor-before-registration.tmp').write_text(str(child.pid))
        Path('/work/monitor-before-registration.tmp').replace('/work/monitor-before-registration')
    else:
        child = subprocess.Popen(['python3', '/monitor.py', str(pidfd)], pass_fds=(pidfd,))
    child.wait(timeout=90)
    raise AssertionError('monitor finished before guardian-death injection')
finally:
    os.close(pidfd)
