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
# Completion requires no surviving descendant, rather than letting namespace
# exit silently clean up leaked processes after successful publication.
children = Path('/proc/self/task/1/children').read_text().split()
assert not children, children
try:
    os.waitpid(-1, os.WNOHANG)
except ChildProcessError:
    pass
else:
    raise AssertionError('unexpected descendant after successful build')
assert Path('/work/published/inventory.cbor').is_file()
print('OUTER_PID_INIT_COMPLETE_BUILD_NO_DESCENDANTS_PASS', flush=True)
