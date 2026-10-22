#!/usr/bin/python3
"""Give the trusted supervisor monitor a pinned handle to this guardian."""
import os
import subprocess

pidfd = os.pidfd_open(os.getpid())
try:
    child = subprocess.Popen(['python3', '/monitor.py', str(pidfd)], pass_fds=(pidfd,))
    child.wait(timeout=90)
    raise AssertionError('monitor finished before guardian-death injection')
finally:
    os.close(pidfd)
