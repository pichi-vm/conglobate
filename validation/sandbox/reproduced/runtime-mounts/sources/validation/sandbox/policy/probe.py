#!/usr/bin/python3
"""Bounded syscall assertions in a disposable guest; no exploit payloads."""
import ctypes
import errno
import fcntl
import json
import os
import pathlib
import pty
import socket
import struct
import subprocess
import termios
import threading

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
key = int(pathlib.Path("/known-key").read_text())
buf = ctypes.create_string_buffer(64)
assert libc.syscall(250, 11, key, buf, 64) == -1 and ctypes.get_errno() == errno.EPERM
print("POLICY_REAL_KEY_READ_DENIED_PASS", flush=True)
status = pathlib.Path("/proc/self/status").read_text().splitlines()
fields = dict(line.split(":", 1) for line in status)
assert int(fields["NoNewPrivs"]) == 1
assert int(fields["Seccomp"]) == 2
expected = sum(1 << n for n in (0, 1, 3, 4, 5, 6, 7, 8, 10, 18, 21, 31))
for field in ("CapBnd", "CapEff", "CapPrm"):
    assert int(fields[field], 16) == expected, (field, fields[field])
assert int(fields["CapAmb"], 16) == int(fields["CapInh"], 16) == 0
assert set(os.environ) <= {"HOME", "PATH", "LC_CTYPE"}
for fd in (9, 257, 258, 259, 270, 271, 272, 273, 274):
    try:
        os.fstat(fd)
    except OSError as error:
        assert error.errno == errno.EBADF
    else:
        raise AssertionError(f"inherited descriptor {fd}")
assert not pathlib.Path("/.old/canary").exists()
assert not pathlib.Path("/supervisor.sock").exists()
assert not pathlib.Path("/dev/shm/supervisor-canary").exists()
print("POLICY_INHERITED_IPC_HANDLES_CLOSED_PASS", flush=True)
print("POLICY_IDENTITY_ENV_FDS_PASS", flush=True)

# x86_64 fixture: calls use null arguments deliberately. EPERM must precede
# argument validation; none of these requests creates or changes a resource.
assert os.uname().machine == "x86_64"
for name, number in (
    ("mount", 165), ("umount2", 166), ("pivot_root", 155),
    ("fsopen", 430), ("fsconfig", 431), ("fsmount", 432),
    ("open_tree", 428), ("move_mount", 429), ("mount_setattr", 442),
    ("unshare", 272), ("setns", 308), ("ptrace", 101),
    ("process_vm_readv", 310), ("process_vm_writev", 311),
    ("pidfd_getfd", 438), ("keyctl", 250), ("add_key", 248),
    ("request_key", 249), ("bpf", 321), ("perf_event_open", 298),
    ("userfaultfd", 323), ("io_uring_setup", 425),
):
    ctypes.set_errno(0)
    result = libc.syscall(number, 0, 0, 0, 0, 0, 0)
    assert result == -1 and ctypes.get_errno() == errno.EPERM, (
        name, result, ctypes.get_errno())
for flag in (0x10000000, 0x00020000, 0x20000000, 0x40000000,
             0x08000000, 0x04000000, 0x02000000):
    result = libc.syscall(56, flag, 0, 0, 0, 0)
    assert result == -1 and ctypes.get_errno() == errno.EPERM
assert libc.syscall(435, 0, 0) == -1 and ctypes.get_errno() == errno.ENOSYS
# A harmless x32 getpid request must hit the explicit ABI gate, including
# kernels that otherwise return ENOSYS because x32 is disabled.
assert libc.syscall(0x40000000 | 39) == -1 and ctypes.get_errno() == errno.EPERM
print("POLICY_NAMESPACE_MOUNT_PROCESS_KEY_DENIALS_PASS", flush=True)

for family, kind, protocol in (
    (socket.AF_INET, socket.SOCK_RAW, socket.IPPROTO_ICMP),
    (socket.AF_PACKET, socket.SOCK_RAW, 0),
):
    try:
        socket.socket(family, kind, protocol)
    except PermissionError:
        pass
    else:
        raise AssertionError("raw socket unexpectedly permitted")
print("POLICY_RAW_SOCKET_DENIALS_PASS", flush=True)

# Socket creation only: no connection to any outside endpoint and no messages
# to kernel control interfaces. Record availability before narrowing policy.
socket_cases = [
    ("unix", socket.AF_UNIX, socket.SOCK_STREAM, 0),
    ("inet", socket.AF_INET, socket.SOCK_STREAM, 0),
    ("inet6", socket.AF_INET6, socket.SOCK_STREAM, 0),
    ("route", socket.AF_NETLINK, socket.SOCK_RAW, 0),
    ("usersock", socket.AF_NETLINK, socket.SOCK_RAW, 2),
    ("audit", socket.AF_NETLINK, socket.SOCK_RAW, 9),
    ("uevent", socket.AF_NETLINK, socket.SOCK_RAW, 15),
    ("generic", socket.AF_NETLINK, socket.SOCK_RAW, 16),
    ("vsock", 40, socket.SOCK_STREAM, 0),
    ("alg", 38, socket.SOCK_SEQPACKET, 0),
    ("bluetooth", 31, socket.SOCK_RAW, 0),
    ("can", 29, socket.SOCK_RAW, 1),
]
socket_outcomes = {}
for name, family, kind, protocol in socket_cases:
    try:
        created = socket.socket(family, kind, protocol)
    except OSError as error:
        socket_outcomes[name] = errno.errorcode[error.errno]
    else:
        created.close()
        socket_outcomes[name] = "created"
print("POLICY_SOCKET_FAMILIES " + json.dumps(socket_outcomes, sort_keys=True), flush=True)
for name in ("unix", "inet", "inet6", "route"):
    assert socket_outcomes[name] == "created", (name, socket_outcomes[name])
for name in ("usersock", "audit", "uevent", "generic", "vsock", "alg", "bluetooth", "can"):
    assert socket_outcomes[name] == "EPERM", (name, socket_outcomes[name])
for kind in (socket.SOCK_STREAM, socket.SOCK_DGRAM, socket.SOCK_SEQPACKET):
    first, second = socket.socketpair(socket.AF_UNIX, kind)
    first.send(b"local-fixture")
    assert second.recv(32) == b"local-fixture"
    first.close()
    second.close()
    # CLOEXEC/NONBLOCK must not change the socket-family decision.
    for family in (38, 40):
        try:
            socket.socket(family, kind | socket.SOCK_CLOEXEC | socket.SOCK_NONBLOCK)
        except OSError as error:
            assert error.errno == errno.EPERM
        else:
            raise AssertionError("optional socket family permitted")
# Null pair output pointer is intentional: the filter must reject first.
for family in (socket.AF_INET, socket.AF_NETLINK, 40):
    assert libc.syscall(53, family, socket.SOCK_STREAM, 0, 0) == -1
    assert ctypes.get_errno() == errno.EPERM
for family in range(64):
    if family in (socket.AF_UNIX, socket.AF_INET, socket.AF_INET6, socket.AF_NETLINK):
        continue
    assert libc.syscall(41, family, socket.SOCK_DGRAM, 0) == -1
    assert ctypes.get_errno() == errno.EPERM, family
for family in (socket.AF_INET, socket.AF_INET6):
    kind = socket.SOCK_RAW | socket.SOCK_CLOEXEC | socket.SOCK_NONBLOCK
    assert libc.syscall(41, family, kind, 0) == -1
    assert ctypes.get_errno() == errno.EPERM
# Kernel socket arguments are ints: unrelated high register bits must not
# change matching of the low 32-bit family/protocol.
assert libc.syscall(41, ctypes.c_ulonglong((1 << 32) | 38), socket.SOCK_SEQPACKET, 0) == -1
assert ctypes.get_errno() == errno.EPERM
assert libc.syscall(41, socket.AF_NETLINK, socket.SOCK_RAW,
                    ctypes.c_ulonglong((1 << 32) | 9)) == -1
assert ctypes.get_errno() == errno.EPERM
# Query only the isolated namespace's links, using a standard route dump.
with socket.socket(socket.AF_NETLINK, socket.SOCK_RAW, 0) as route:
    route.settimeout(2)
    route.bind((0, 0))
    route.send(struct.pack("IHHII", 32, 18, 0x301, 1, 0) + bytes(16))
    reply = route.recv(8192)
    assert len(reply) >= 16 and struct.unpack_from("H", reply, 4)[0] == 16
# No external interfaces in this fixture. Even ordinary IP traffic cannot leave.
for family, address in ((socket.AF_INET, ("192.0.2.1", 9)),
                        (socket.AF_INET6, ("2001:db8::1", 9))):
    with socket.socket(family, socket.SOCK_DGRAM) as udp:
        try:
            udp.sendto(b"bounded-fixture", address)
        except OSError as error:
            assert error.errno in (errno.ENETUNREACH, errno.EADDRNOTAVAIL)
        else:
            raise AssertionError("disabled namespace routed a datagram")
print("POLICY_SOCKET_ALLOWLIST_AND_DISABLED_NETWORK_PASS", flush=True)

# Exercise only the private terminal and the process's own namespace handles.
# No host terminal, host device, or supervisor namespace is opened.
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
assert struct.unpack("HHHH", fcntl.ioctl(slave, termios.TIOCGWINSZ, b"\0" * 8))[:2] == (24, 80)
os.close(master)
os.close(slave)
assert not pathlib.Path("/sys").exists()
assert sorted(p.name for p in pathlib.Path("/dev").iterdir()) == ["null", "ptmx", "pts"]
with open("/dev/null", "w") as stream:
    stream.write("device fixture\n")
fd = os.open("/proc/self/ns/user", os.O_RDONLY)
try:
    try:
        fcntl.ioctl(fd, 0xb702)  # NS_GET_PARENT
    except PermissionError:
        pass
    else:
        raise AssertionError("ancestor user namespace handle obtained")
finally:
    os.close(fd)
print("POLICY_PRIVATE_DEVPTS_AND_NAMESPACE_HANDLE_PASS", flush=True)

# Ordinary commands retain required owner/permission operations and can fork
# and create threads. These are smoke checks, not a package-build qualification.
path = pathlib.Path("/tmp/owned")
path.write_text("bounded fixture\n")
os.chown(path, 1234, 1234)
os.chmod(path, 0o640)
assert path.stat().st_uid == 1234
assert path.read_text() == "bounded fixture\n"
os.link(path, "/tmp/link")
os.symlink("owned", "/tmp/symlink")
assert pathlib.Path("/tmp/symlink").read_text() == path.read_text()
done = []
thread = threading.Thread(target=lambda: done.append(True))
thread.start()
thread.join(timeout=2)
assert done == [True]
subprocess.run(["/bin/bash", "-eu", "-c", "test -r /tmp/owned"], check=True, timeout=2)
path.unlink()
pathlib.Path("/tmp/link").unlink()
pathlib.Path("/tmp/symlink").unlink()
print("POLICY_NORMAL_FILE_FORK_THREAD_PASS", flush=True)


# Opens only: no writes to kernel control files and no privileged ioctls.
protected = ("/proc/sysrq-trigger", "/proc/sys/kernel/core_pattern",
             "/proc/sys/kernel/modprobe", "/proc/sys/vm/drop_caches")
for path in protected:
    try:
        fd = os.open(path, os.O_WRONLY | os.O_CLOEXEC)
    except OSError as error:
        assert error.errno in (errno.EACCES, errno.EPERM, errno.EROFS), (path, error)
    else:
        os.close(fd)
        raise AssertionError(f"global proc control writable: {path}")
print("POLICY_GLOBAL_PROC_CONTROL_OPENS_DENIED_PASS", flush=True)
mounts = {}
for line in pathlib.Path("/proc/self/mountinfo").read_text().splitlines():
    fields = line.split()
    mounts[fields[4]] = set(fields[5].split(','))
assert {"nosuid", "nodev", "noexec"} <= mounts["/proc"]
assert {"nosuid", "noexec"} <= mounts["/dev"]
assert {"nosuid", "noexec"} <= mounts["/dev/pts"]
assert not pathlib.Path("/dev/step-canary").exists()
pathlib.Path("/dev/step-canary").write_text("disposable runtime file")
print("POLICY_RUNTIME_MOUNTS_PRIVATE_AND_RESTRICTED_PASS", flush=True)
