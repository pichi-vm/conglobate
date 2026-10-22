#!/usr/bin/python3
"""Single-threaded trusted launcher for the ownership-init validation helper."""
import ctypes
import os
import subprocess
fd = os.pidfd_open(os.getpid())
# Changes only this disposable fixture process and its future child.
assert ctypes.CDLL(None, use_errno=True).unshare(0x20000000 | 0x00020000) == 0
child = subprocess.Popen(['/ownership-init'], stdin=fd)
os.close(fd)
result = child.wait(timeout=100)
raise SystemExit(result if result >= 0 else 128 - result)
