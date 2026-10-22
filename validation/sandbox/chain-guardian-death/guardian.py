#!/usr/bin/python3
"""Trusted test guardian: owns the build cgroup until the observer ends it."""
import os
from pathlib import Path
import signal
import subprocess
import time

group = Path('/sys/fs/cgroup/guardian-death-fixture')
group.mkdir()
supervisor = subprocess.Popen(['/bin/bash', '-c',
    'kill -STOP $$; exec unshare --mount /publication-helper --publish'])
pidfd = os.pidfd_open(supervisor.pid)
pid, status = os.waitpid(supervisor.pid, os.WUNTRACED)
assert pid == supervisor.pid and os.WIFSTOPPED(status)
(group / 'cgroup.procs').write_text(str(supervisor.pid))
signal.pidfd_send_signal(pidfd, signal.SIGCONT)
Path('/work/guardian-ready').write_text(str(supervisor.pid))
# This deliberately models only the existing one-way guardian arrangement:
# no supervisor watch of guardian lifetime is supplied by this fixture.
try:
    supervisor.wait(timeout=90)
    raise AssertionError('supervisor finished before guardian-death injection')
finally:
    (group / 'cgroup.kill').write_text('1')
    os.close(pidfd)
