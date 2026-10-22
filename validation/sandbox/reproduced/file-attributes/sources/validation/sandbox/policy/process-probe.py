#!/usr/bin/python3
"""Valid-argument controls using only this probe's buffers, children and FDs."""
import ctypes
import errno
import os
import sys

assert os.uname().machine == "x86_64"
restricted = sys.argv[1:] == ["--restricted"]
assert restricted or sys.argv[1:] == []
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long


def call(number, *args):
    # syscall is variadic: pass full-width scalar arguments, including flags.
    return libc.syscall(ctypes.c_long(number),
                        *(ctypes.c_long(a) if isinstance(a, int) else a for a in args))


class Iovec(ctypes.Structure):
    _fields_ = [("base", ctypes.c_void_p), ("length", ctypes.c_size_t)]


def check(result, expected):
    if restricted:
        assert result == -1 and ctypes.get_errno() == errno.EPERM, (
            result, ctypes.get_errno())
    else:
        assert result == expected, (result, expected, ctypes.get_errno())


payload = b"disposable-process-canary"
source = ctypes.create_string_buffer(payload)
destination = ctypes.create_string_buffer(len(payload))
local = Iovec(ctypes.addressof(destination), len(payload))
remote = Iovec(ctypes.addressof(source), len(payload))
check(call(310, os.getpid(), ctypes.byref(local), 1,
                   ctypes.byref(remote), 1, 0), len(payload))
assert destination.raw == (bytes(len(payload)) if restricted else payload)

# Only this process's fresh scratch buffer is a write target.
destination = ctypes.create_string_buffer(len(payload))
local = Iovec(ctypes.addressof(source), len(payload))
remote = Iovec(ctypes.addressof(destination), len(payload))
check(call(311, os.getpid(), ctypes.byref(local), 1,
                   ctypes.byref(remote), 1, 0), len(payload))
assert destination.raw == (bytes(len(payload)) if restricted else payload)

fd = os.memfd_create("process-probe-canary", os.MFD_CLOEXEC)
os.write(fd, payload)
pidfd = os.pidfd_open(os.getpid())
duplicate = call(438, pidfd, fd, 0)
if restricted:
    check(duplicate, 0)
else:
    assert duplicate >= 0, ctypes.get_errno()
    assert os.pread(duplicate, len(payload), 0) == payload
    os.close(duplicate)
assert os.pread(fd, len(payload), 0) == payload
os.close(pidfd)
os.close(fd)

# PTRACE_TRACEME names no other process. A fresh owned child asks its parent
# to trace it, then exits without an exec or stop. No supervisor is traced.
child = os.fork()
if child == 0:
    result = call(101, 0, 0, 0, 0)
    expected = (result == -1 and ctypes.get_errno() == errno.EPERM
                if restricted else result == 0)
    os._exit(0 if expected else 1)
waited, status = os.waitpid(child, 0)
assert waited == child and os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0
assert source.value == payload
print("PROCESS_VALID_ARGUMENT_" + ("DENIALS_PASS" if restricted else "CONTROLS_PASS"), flush=True)
