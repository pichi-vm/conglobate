#!/usr/bin/python3
"""Guest-only access checks against the parent's dedicated disposable canary."""
import ctypes
import errno
import os
from pathlib import Path
import subprocess
import sys

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long


def call(number, *args):
    return libc.syscall(ctypes.c_long(number),
                        *(ctypes.c_long(a) if isinstance(a, int) else a for a in args))


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


payload = b"dedicated-parent-canary"
if len(sys.argv) == 1:
    assert os.getuid() == 0 and os.uname().machine == "x86_64"
    canary = ctypes.create_string_buffer(payload)
    fd = os.memfd_create("parent-canary", os.MFD_CLOEXEC)
    os.write(fd, payload)
    arguments = [str(os.getpid()), str(ctypes.addressof(canary)), str(fd)]
    inspector = ["/usr/bin/python3", "/cross-process.py"]
    subprocess.run(inspector + ["control"] + arguments, check=True, timeout=5)
    subprocess.run(["unshare", "--user", "--map-users", "0:100000:65536",
                    "--map-groups", "0:100000:65536", "--setuid", "0",
                    "--setgid", "0"] + inspector + ["mapped"] + arguments,
                   check=True, timeout=5)
    assert canary.value == payload and os.pread(fd, len(payload), 0) == payload
    os.close(fd)
    print("CROSS_PROCESS_PARENT_CANARIES_UNCHANGED_PASS", flush=True)
    sys.exit(0)

mode, pid, address, fd = sys.argv[1:]
pid, address, fd = int(pid), int(address), int(fd)
assert mode in ("control", "mapped")
denied = mode == "mapped"
assert os.getuid() == 0
assert Path(f"/proc/{pid}/status").exists()  # Deliberately no PID isolation.
if denied:
    assert Path("/proc/self/uid_map").read_text().split() == ["0", "100000", "65536"]


def check(result, expected):
    if denied:
        assert result == -1 and ctypes.get_errno() == errno.EPERM, (result, ctypes.get_errno())
    else:
        assert result == expected, (result, ctypes.get_errno())


scratch = ctypes.create_string_buffer(len(payload))
local = Iovec(ctypes.addressof(scratch), len(payload))
remote = Iovec(address, len(payload))
check(call(310, pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0), len(payload))
assert scratch.raw == (bytes(len(payload)) if denied else payload)
# Write only the same fixed canary bytes back to the parent's dedicated buffer.
source = ctypes.create_string_buffer(payload)
local = Iovec(ctypes.addressof(source), len(payload))
check(call(311, pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0), len(payload))
pidfd = os.pidfd_open(pid)
duplicate = call(438, pidfd, fd, 0)
if denied:
    check(duplicate, 0)
else:
    assert duplicate >= 0, ctypes.get_errno()
    assert os.pread(duplicate, len(payload), 0) == payload
    os.close(duplicate)
os.close(pidfd)
try:
    os.kill(pid, 0)  # Permission query only; sends no signal.
except OSError as error:
    assert denied and error.errno == errno.EPERM
else:
    assert not denied
for kind in ("user", "mnt", "pid", "net"):
    try:
        handle = os.open(f"/proc/{pid}/ns/{kind}", os.O_RDONLY | os.O_CLOEXEC)
    except OSError as error:
        assert denied and error.errno in (errno.EACCES, errno.EPERM), (kind, error)
    else:
        os.close(handle)
        assert not denied, kind
print("CROSS_PROCESS_" + ("MAPPED_DENIALS_PASS" if denied else "VALID_CONTROLS_PASS"), flush=True)
