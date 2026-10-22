#!/usr/bin/python3
"""Trusted fixture supervisor; blocked before it can create any descendants."""
import os
import pathlib
import signal
import subprocess
import sys
import time

stage = sys.argv[1]
# Test-only launch gate. The guardian assigns this process to its cgroup and
# acquires the pidfd before allowing any descendant creation.
os.kill(os.getpid(), signal.SIGSTOP)
if stage == "prelaunch":
    pathlib.Path("/work/phase").write_text(stage)
    time.sleep(10)
else:
    command = [
        "unshare", "--user", "--mount", "--pid", "--net", "--ipc", "--uts", "--fork",
        "--map-users", "0:100000:65536", "--map-groups", "0:100000:65536",
        "--setuid", "0", "--setgid", "0", "/bin/bash", "/launch", stage,
    ]
    subprocess.run(command, check=True, timeout=12)
