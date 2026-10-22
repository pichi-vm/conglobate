#!/usr/bin/python3
import os
from pathlib import Path
import subprocess
import time

group=Path('/sys/fs/cgroup/limited-command')
group.mkdir()
(group/'pids.max').write_text('1')
child=subprocess.Popen(['/publication-helper','--publish'])
ready=Path('/work/command.pid')
deadline=time.monotonic()+60
while not ready.exists() or not ready.read_text().strip():
    assert child.poll() is None
    assert time.monotonic()<deadline,'command handoff timeout'
    time.sleep(.01)
pid=int(ready.read_text())
pidfd=os.pidfd_open(pid)
# Trusted pre-exec gate precedes all image code. No root cgroup file is
# available after entry; worker/supervisor remain outside the limited group.
(group/'cgroup.procs').write_text(str(pid))
Path('/work/command-go').touch()
assert child.wait(timeout=30)==101
os.close(pidfd)
events=dict(line.split() for line in (group/'pids.events').read_text().splitlines())
assert int(events['max'])>=1, events
assert (group/'cgroup.procs').read_text()==''
group.rmdir()
print('CHAIN_PIDS_LIMIT_EVENT_AND_CGROUP_CLEANUP_PASS',flush=True)
