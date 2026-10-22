#!/usr/bin/python3
import os
from pathlib import Path
import signal
import subprocess
import time

def until(predicate, seconds=10):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        if predicate(): return
        time.sleep(.01)
    raise AssertionError('worker OOM controller timeout')

group=Path('/sys/fs/cgroup/limited-worker')
group.mkdir()
(group/'memory.max').write_text(str(128*1024*1024))
(group/'memory.swap.max').write_text('0')
child=subprocess.Popen(['/publication-helper','--publish'])
pidfile=Path('/work/daemon.pid')
until(lambda: pidfile.exists() and pidfile.read_text().strip(),60)
pid=int(pidfile.read_text())
pidfd=os.pidfd_open(pid)
until(lambda: '\nState:\tT' in Path(f'/proc/{pid}/status').read_text())
(group/'cgroup.procs').write_text(str(pid))
signal.pidfd_send_signal(pidfd,signal.SIGCONT)
until(lambda: Path('/work/worker-stopped').exists())
connections=list(Path('/sys/fs/fuse/connections').iterdir())
assert len(connections)==1
connection=connections[0]
abort=os.open(connection/'abort',os.O_WRONLY)
until(lambda: int((connection/'waiting').read_text())>0)
assert int((group/'memory.current').read_text())>0
print('WORKER_OOM_REQUEST_QUEUED',flush=True)
start=time.monotonic()
# The daemon alone is constrained. Kernel accounting and OOM events determine
# the result; the controller does not send SIGKILL to the daemon.
(group/'memory.max').write_text('0')
def oom():
    events=dict(line.split() for line in (group/'memory.events').read_text().splitlines())
    return int(events['oom_kill'])>=1
until(oom)
print('WORKER_OOM_KERNEL_KILL_EVENT_CONFIRMED',flush=True)
Path('/work/cancel-now').touch()
until(lambda: Path('/work/clients-signaled').exists())
os.write(abort,b'1')
os.close(abort)
Path('/work/abort-done').touch()
assert child.wait(timeout=10)==101
os.close(pidfd)
assert (group/'cgroup.procs').read_text()==''
group.rmdir()
assert time.monotonic()-start<15
print('WORKER_OOM_SUPERVISOR_SURVIVED_CGROUP_CLEANUP_PASS',flush=True)
