#!/usr/bin/python3
"""Bounded sibling-userns checks against a disposable worker-shaped canary."""
import ctypes
import errno
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long


def call(number, *args):
    return libc.syscall(ctypes.c_long(number),
                        *(ctypes.c_long(a) if isinstance(a, int) else a for a in args))


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


payload = b"worker-identity-canary"

if sys.argv[1:2] == ["isolated-pid"]:
    inherited = int(sys.argv[2])
    os.fstat(inherited)
    try:
        signal.pidfd_send_signal(inherited, 0)
    except OSError as error:
        assert error.errno == errno.EINVAL, error
    else:
        raise AssertionError("ancestor worker signal permission crossed PID namespace")
    print("WORKER_IDENTITY_ANCESTOR_PIDFD_DENIED_PASS", flush=True)
    sys.exit(0)


def namespace(uid):
    return ["unshare", "--user", "--map-users", f"0:{uid}:65536",
            "--map-groups", f"0:{uid}:65536", "--setuid", "0", "--setgid", "0"]


if sys.argv[1:] == ["worker"]:
    signal.alarm(25)
    canary = ctypes.create_string_buffer(payload)
    fd = os.memfd_create("identity-canary", os.MFD_CLOEXEC)
    os.write(fd, payload)
    print(json.dumps({"pid": os.getpid(), "address": ctypes.addressof(canary),
                      "fd": fd, "userns": os.readlink("/proc/self/ns/user")}), flush=True)
    assert sys.stdin.readline() == "done\n"
    assert canary.value == payload and os.pread(fd, len(payload), 0) == payload
    print("WORKER_IDENTITY_CANARIES_UNCHANGED_PASS", flush=True)
    sys.exit(0)

if len(sys.argv) == 1:
    assert os.getuid() == 0 and os.uname().machine == "x86_64"
    script = ["/usr/bin/python3", "/worker-identity.py"]
    worker = subprocess.Popen(namespace(100000) + script + ["worker"],
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    try:
        info = json.loads(worker.stdout.readline())
        shared = ["nsenter", f"--user=/proc/{info['pid']}/ns/user", "--setuid", "0",
                  "--setgid", "0", "--"]
        for mode, prefix in [("root", []), ("same", namespace(100000)),
                             ("shared", shared),
                             ("different", namespace(200000))]:
            subprocess.run(prefix + script + [mode, json.dumps(info)], check=True, timeout=5)
        pidfd = os.pidfd_open(info["pid"])
        try:
            subprocess.run(namespace(100000) + ["--pid", "--fork"] + script +
                           ["isolated-pid", str(pidfd)], pass_fds=(pidfd,),
                           check=True, timeout=5)
            subprocess.run(shared + ["unshare", "--pid", "--fork"] + script +
                           ["isolated-pid", str(pidfd)], pass_fds=(pidfd,),
                           check=True, timeout=5)
        finally:
            os.close(pidfd)
        output, _ = worker.communicate("done\n", timeout=5)
        assert worker.returncode == 0 and output.strip() == "WORKER_IDENTITY_CANARIES_UNCHANGED_PASS"
        print(output, end="", flush=True)
    finally:
        if worker.poll() is None:
            worker.kill()
            worker.wait(timeout=5)
    print("WORKER_IDENTITY_MATRIX_PASS", flush=True)
    sys.exit(0)

mode, serialized = sys.argv[1:]
info = json.loads(serialized)
pid = info["pid"]
assert Path(f"/proc/{pid}/status").exists()
assert (os.readlink("/proc/self/ns/user") == info["userns"]) == (mode == "shared")
permitted = mode in ("root", "shared")
scratch = ctypes.create_string_buffer(len(payload))
local = Iovec(ctypes.addressof(scratch), len(payload))
remote = Iovec(info["address"], len(payload))
result = call(310, pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
outcomes = {"memory_read": "allowed" if result >= 0 else errno.errorcode[ctypes.get_errno()]}
assert result == len(payload) if permitted else result == -1 and ctypes.get_errno() == errno.EPERM
assert scratch.raw == (payload if permitted else bytes(len(payload)))
pidfd = os.pidfd_open(pid)
duplicate = call(438, pidfd, info["fd"], 0)
outcomes["getfd"] = "allowed" if duplicate >= 0 else errno.errorcode[ctypes.get_errno()]
if permitted:
    assert duplicate >= 0
    assert os.pread(duplicate, len(payload), 0) == payload
    os.close(duplicate)
else:
    assert duplicate == -1 and ctypes.get_errno() == errno.EPERM
for name, action in [("kill_zero", lambda: os.kill(pid, 0)),
                     ("pidfd_signal_zero", lambda: signal.pidfd_send_signal(pidfd, 0))]:
    try:
        action()  # Permission queries only; no signal is delivered.
    except OSError as error:
        outcomes[name] = errno.errorcode[error.errno]
        assert mode == "different" and error.errno == errno.EPERM
    else:
        outcomes[name] = "allowed"
        assert mode in ("root", "same", "shared")
os.close(pidfd)
print("WORKER_IDENTITY_RESULT " + mode + " " + json.dumps(outcomes, sort_keys=True), flush=True)
