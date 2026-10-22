#!/usr/bin/python3
"""Cancel guest-only launcher instances; retain pidfds for precise cleanup."""
import ctypes
import os
import pathlib
import select
import signal
import subprocess
import time

libc = ctypes.CDLL(None, use_errno=True)
assert libc.prctl(36, 1, 0, 0, 0) == 0  # PR_SET_CHILD_SUBREAPER, test instrumentation.
phase = pathlib.Path("/work/phase")
heartbeat = pathlib.Path("/work/root/tmp/heartbeat")
mounts = pathlib.Path("/proc/self/mountinfo").read_text()


def until(predicate, seconds=3):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.01)
    raise AssertionError("bounded lifecycle condition timed out")


def children(pid):
    path = pathlib.Path(f"/proc/{pid}/task/{pid}/children")
    return [int(value) for value in path.read_text().split()] if path.exists() else []


for mode, stage in [("none", "command"), ("option", "command")] + [
    ("supervised", stage) for stage in (
        "after-userns", "after-root-bind", "after-proc", "before-entry", "command")
]:
    phase.write_text("")
    heartbeat.unlink(missing_ok=True)
    command = [
        "unshare", "--user", "--mount", "--pid", "--net", "--ipc", "--uts", "--fork",
        "--map-users", "0:100000:65536", "--map-groups", "0:100000:65536",
        "--setuid", "0", "--setgid", "0",
    ]
    if mode == "option":
        command.append("--kill-child=KILL")
    command += ["/bin/bash", "/launch", stage]
    launcher = subprocess.Popen(command)
    init_fd = None
    try:
        if stage == "command":
            until(lambda: heartbeat.exists() and heartbeat.stat().st_size >= 2)
        else:
            until(lambda: phase.read_text().strip() == stage)
        pids = children(launcher.pid)
        assert len(pids) == 1, pids
        init_fd = os.pidfd_open(pids[0])
        # The tracked init owns the command PID namespace. SIGKILL is delivered
        # only to this fixture's launcher; the retained pidfd enables cleanup.
        launcher.kill()
        assert launcher.wait(timeout=2) == -signal.SIGKILL
        if mode != "supervised":
            before = heartbeat.stat().st_size
            until(lambda: heartbeat.stat().st_size > before)
            marker = ("LAUNCHER_DEATH_WITHOUT_CHILD_KILL_LEAVES_WRITER" if mode == "none"
                      else "KILL_CHILD_WITH_ID_CHANGE_LEAVES_WRITER")
            print(marker, flush=True)
        # Cancellation belongs to the trusted supervisor, with a stable init
        # handle acquired before accepting command output.
        signal.pidfd_send_signal(init_fd, signal.SIGKILL)
        poll = select.poll()
        poll.register(init_fd, select.POLLIN)
        if not poll.poll(100):
            inspect = children(os.getpid())
            for child in inspect:
                inspect.extend(children(child))
                state = pathlib.Path(f"/proc/{child}/status").read_text()
                state = [line for line in state.splitlines()
                         if line.startswith(("State:", "PPid:", "NSpid:"))]
                wait = pathlib.Path(f"/proc/{child}/wchan").read_text()
                cmd = pathlib.Path(f"/proc/{child}/cmdline").read_bytes()
                print("LIFECYCLE_WAIT_STATE", child, state, wait, cmd, flush=True)
        # Reap adopted init/descendants. No fixture child may survive cancellation.
        deadline = time.monotonic() + 2
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                break
            assert time.monotonic() < deadline, "unreaped fixture descendants"
            if pid == 0:
                time.sleep(.01)
        assert not children(os.getpid())
        assert poll.poll(2000), "PID namespace init did not terminate after reaping"
        size = heartbeat.stat().st_size if heartbeat.exists() else 0
        time.sleep(.1)
        assert (heartbeat.stat().st_size if heartbeat.exists() else 0) == size
        assert pathlib.Path("/proc/self/mountinfo").read_text() == mounts
        if mode == "supervised":
            print("LIFECYCLE_CANCEL_PASS", stage, flush=True)
    finally:
        if init_fd is not None:
            try:
                signal.pidfd_send_signal(init_fd, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(init_fd)
        if launcher.poll() is None:
            launcher.kill()
            launcher.wait(timeout=2)
