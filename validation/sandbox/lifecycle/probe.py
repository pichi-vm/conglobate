#!/usr/bin/python3
"""Finite synthetic writer and detached descendant, inside disposable PID ns."""
import os
import pathlib
import time

assert os.getpid() == 1
status = dict(line.split(":", 1) for line in pathlib.Path("/proc/self/status").read_text().splitlines())
assert int(status["Seccomp"]) == 2 and int(status["NoNewPrivs"]) == 1
pid = os.fork()
if pid == 0:
    os.setsid()
    child = os.fork()
    if child:
        os._exit(0)
    with open("/tmp/heartbeat", "ab", buffering=0) as stream:
        for _ in range(500):  # Hard bound even if test cleanup fails.
            stream.write(b".")
            time.sleep(.02)
    os._exit(0)
os.waitpid(pid, 0)
for _ in range(500):
    time.sleep(.02)
