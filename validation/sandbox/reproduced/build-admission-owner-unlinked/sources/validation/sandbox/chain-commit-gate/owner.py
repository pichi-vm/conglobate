#!/usr/bin/python3
"""Trusted prototype PID init for an ordinary complete build."""
import os
from pathlib import Path
import subprocess
import time

assert os.getpid() == 1
assert Path('/proc/self/stat').read_text().split()[0] == '1'
child = subprocess.Popen(['/publication-helper', '--publish'])
assert child.wait(timeout=90) == 0
if Path('/completion-mode').read_text().strip() == 'live-descendant':
    # Harmless, bounded trusted fixture process; no recipe or host interaction.
    sleeper = subprocess.Popen(['/usr/bin/sleep', '30'])
    Path('/work/late-child-local').write_text(str(sleeper.pid))
    deadline = time.monotonic() + 5
    while not Path('/work/late-child-pinned').exists():
        assert time.monotonic() < deadline, 'observer did not pin fixture child'
        time.sleep(.01)
    assert sleeper.poll() is None
# Completion requires no surviving descendant, rather than letting namespace
# exit silently clean up leaked processes after successful publication.
children = Path('/proc/self/task/1/children').read_text().split()
for pid in children:
    status = Path(f'/proc/{pid}/status').read_text()
    print('OUTER_PID_ADOPTED_CHILD_STATE', pid,
          next(line for line in status.splitlines() if line.startswith('State:')), flush=True)
    if '\nState:\tZ' not in status:
        print('LIVE_DESCENDANT_REJECTED_BEFORE_PUBLIC_COMMIT', pid, flush=True)
    assert '\nState:\tZ' in status, f'live descendant remains: {pid}'
reaped = 0
while True:
    try:
        pid, status = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        break
    assert pid > 0, 'live descendant remains after build completion'
    assert os.WIFEXITED(status) or os.WIFSIGNALED(status)
    reaped += 1
assert not Path('/proc/self/task/1/children').read_text().split()
print('OUTER_PID_ADOPTED_EXITED_CHILDREN_REAPED', reaped, flush=True)
assert Path('/work/published/inventory.cbor').is_file()
print('OUTER_PID_INIT_COMPLETE_BUILD_NO_DESCENDANTS_PASS', flush=True)
if Path('/completion-mode').read_text().strip() == 'late-failure':
    print('INJECTED_LATE_INIT_COMPLETION_FAILURE', flush=True)
    raise SystemExit(17)
