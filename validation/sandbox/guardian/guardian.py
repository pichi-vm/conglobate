#!/usr/bin/python3
"""Independent trusted cgroup owner, outside the fixture supervisor's group."""
import ctypes
import os
import pathlib
import select
import signal
import subprocess
import time

libc = ctypes.CDLL(None, use_errno=True)
assert libc.prctl(36, 1, 0, 0, 0) == 0  # Test-only subreaper instrumentation.
phase = pathlib.Path("/work/phase")
heartbeat = pathlib.Path("/work/root/tmp/heartbeat")
mounts = pathlib.Path("/proc/self/mountinfo").read_text()


def until(predicate, seconds=3):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.01)
    raise AssertionError("guardian condition timed out")


def reap():
    deadline = time.monotonic() + 3
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return
        assert time.monotonic() < deadline, "fixture descendants not reaped"
        if pid == 0:
            time.sleep(.01)


for index, (negative, stage) in enumerate([
    (True, "command"), (False, "prelaunch"),
    (False, "after-userns"), (False, "command"),
]):
    group = pathlib.Path(f"/sys/fs/cgroup/fixture-{index}")
    group.mkdir()
    phase.write_text("")
    heartbeat.unlink(missing_ok=True)
    supervisor = subprocess.Popen(["python3", "/supervisor.py", stage])
    pidfd = os.pidfd_open(supervisor.pid)
    try:
        pid, status = os.waitpid(supervisor.pid, os.WUNTRACED)
        assert pid == supervisor.pid and os.WIFSTOPPED(status)
        assert os.WSTOPSIG(status) == signal.SIGSTOP
        # No child can escape group assignment through a fork-before-move race:
        # this trusted supervisor is stopped before its first fork.
        (group / "cgroup.procs").write_text(str(supervisor.pid))
        signal.pidfd_send_signal(pidfd, signal.SIGCONT)
        if stage == "command":
            until(lambda: heartbeat.exists() and heartbeat.stat().st_size >= 2)
        else:
            until(lambda: phase.read_text().strip() == stage)
        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
        poll = select.poll()
        poll.register(pidfd, select.POLLIN)
        assert poll.poll(2000), "guardian failed to observe supervisor death"
        assert supervisor.wait(timeout=2) == -signal.SIGKILL
        if negative:
            before = heartbeat.stat().st_size
            until(lambda: heartbeat.stat().st_size > before)
            print("SUPERVISOR_DEATH_ALONE_LEAVES_WRITER", flush=True)
        # The same action is used for positive cases and negative-control cleanup.
        (group / "cgroup.kill").write_text("1")
        until(lambda: "populated 0" in (group / "cgroup.events").read_text())
        reap()
        size = heartbeat.stat().st_size if heartbeat.exists() else 0
        time.sleep(.1)
        assert (heartbeat.stat().st_size if heartbeat.exists() else 0) == size
        assert (group / "cgroup.procs").read_text() == ""
        assert pathlib.Path("/proc/self/mountinfo").read_text() == mounts
        group.rmdir()
        if not negative:
            print("GUARDIAN_CLEANUP_PASS", stage, flush=True)
    finally:
        if group.exists():
            (group / "cgroup.kill").write_text("1")
        if supervisor.poll() is None:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            supervisor.wait(timeout=2)
        os.close(pidfd)
