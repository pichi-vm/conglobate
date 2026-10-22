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
import stat
import subprocess
import termios
import threading

# Confirm exact fixture-owned descriptors before ordinary stream operations.
stdio = json.loads(pathlib.Path('/stdio-identity').read_text())
for fd, name in ((0, 'stdin'), (1, 'output'), (2, 'output')):
    info = os.fstat(fd)
    assert [info.st_dev, info.st_ino] == stdio[name]
    assert (stat.S_ISCHR(info.st_mode) if fd == 0 else stat.S_ISFIFO(info.st_mode))
    mode = fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE
    assert mode == (os.O_RDONLY if fd == 0 else os.O_WRONLY)
assert os.fstat(0).st_rdev == os.makedev(1, 3)
assert os.read(0, 1) == b''
for fd, token in ((1, b'OWNED_STDOUT_DUP\n'), (2, b'OWNED_STDERR_DUP\n')):
    duplicate = os.dup(fd)
    try:
        assert os.write(duplicate, token) == len(token)
    finally:
        os.close(duplicate)
print('POLICY_OWNED_STDIO_PASS', flush=True)

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
# Self-only valid capability requests: CAP_SETPCAP must not restore a bit
# absent from bounding/permitted sets. No other process is targeted.
class CapabilityHeader(ctypes.Structure):
    _fields_ = [('version', ctypes.c_uint32), ('pid', ctypes.c_int32)]
class CapabilityData(ctypes.Structure):
    _fields_ = [('effective', ctypes.c_uint32), ('permitted', ctypes.c_uint32),
                ('inheritable', ctypes.c_uint32)]
header = CapabilityHeader(0x20080522, 0)
original_caps = (CapabilityData * 2)()
assert libc.syscall(125, ctypes.byref(header), original_caps) == 0
assert original_caps[0].effective | (original_caps[1].effective << 32) == expected
for bit in (1 << 19, 1 << 9):  # absent SYS_PTRACE and LINUX_IMMUTABLE
    for target in ('effective', 'permitted', 'inheritable'):
        attempted = (CapabilityData * 2).from_buffer_copy(original_caps)
        setattr(attempted[0], target, getattr(attempted[0], target) | bit)
        ctypes.set_errno(0)
        assert libc.syscall(126, ctypes.byref(header), attempted) == -1
        assert ctypes.get_errno() == errno.EPERM, (bit, target, ctypes.get_errno())
        observed = (CapabilityData * 2)()
        assert libc.syscall(125, ctypes.byref(header), observed) == 0
        assert bytes(observed) == bytes(original_caps)
for capability in (19, 9):
    assert libc.prctl(23, capability, 0, 0, 0) == 0  # PR_CAPBSET_READ
    ctypes.set_errno(0)
    assert libc.prctl(47, 2, capability, 0, 0) == -1  # PR_CAP_AMBIENT_RAISE
    assert ctypes.get_errno() == errno.EPERM
    assert libc.prctl(47, 1, capability, 0, 0) == 0  # PR_CAP_AMBIENT_IS_SET
print('POLICY_DROPPED_CAPABILITIES_CANNOT_BE_REGAINED_PASS', flush=True)
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
subprocess.run(["/usr/bin/python3", "/credential-test.py", "--restricted"], check=True, timeout=5)

# Bounded owned-object check for a newly added mount API. No mount is attached
# and no attributes outside this private command root are changed.
owned_tree = pathlib.Path('/tmp/owned-open-tree-attr')
owned_tree.mkdir()
try:
    ctypes.set_errno(0)
    treefd = libc.syscall(467, -100, ctypes.c_char_p(os.fsencode(owned_tree)),
                         1 | os.O_CLOEXEC, ctypes.c_void_p(), ctypes.c_size_t(0))
    error = ctypes.get_errno()
    if treefd >= 0:
        os.close(treefd)
    assert treefd == -1 and error == errno.EPERM, ('open_tree_attr', treefd, error)
    print('POLICY_OPEN_TREE_ATTR_DENIED_PASS', flush=True)
finally:
    owned_tree.rmdir()

ctypes.set_errno(0)
contextfd = libc.syscall(433, -100, ctypes.c_char_p(b'/dev'), 1)
error = ctypes.get_errno()
if contextfd >= 0:
    os.close(contextfd)
assert contextfd == -1 and error == errno.EPERM, ('restricted fspick', contextfd, error)
print('POLICY_FSPICK_DENIED_PASS', flush=True)

# Read-only namespace queries on the command's own handles. Numeric IDs are
# compared locally and are not used to acquire or enter any outside namespace.
namespace_ids = {}
for namespace in ('mnt', 'pid', 'net', 'user', 'ipc', 'uts', 'cgroup', 'time'):
    nsfd = os.open('/proc/self/ns/' + namespace, os.O_RDONLY | os.O_CLOEXEC)
    try:
        identifier = bytearray(8)
        assert fcntl.ioctl(nsfd, 0x8008b70d, identifier, True) == 0  # NS_GET_ID
        namespace_ids[namespace] = struct.unpack('=Q', identifier)[0]
        if namespace == 'mnt':
            info = bytearray(struct.pack('=IIQ', 16, 0, 0))
            assert fcntl.ioctl(nsfd, 0x8010b70a, info, True) == 0  # NS_MNT_GET_INFO
            size, mounts, identity = struct.unpack('=IIQ', info)
            assert size == 16 and mounts > 0 and identity == namespace_ids['mnt']
            for request in (0x8010b70b, 0x8010b70c):  # NEXT / PREV
                try:
                    unexpected = fcntl.ioctl(nsfd, request, bytearray(struct.pack('=IIQ',16,0,0)), True)
                except OSError as error:
                    assert error.errno == errno.EPERM, error
                else:
                    if unexpected >= 0:
                        os.close(unexpected)
                    raise AssertionError('neighbor namespace handle unexpectedly returned')
    finally:
        os.close(nsfd)

class NamespaceRequest(ctypes.Structure):
    _fields_ = [('size', ctypes.c_uint32), ('spare', ctypes.c_uint32),
                ('last_id', ctypes.c_uint64), ('kind', ctypes.c_uint32),
                ('spare2', ctypes.c_uint32), ('owner', ctypes.c_uint64)]

assert ctypes.sizeof(NamespaceRequest) == 32
for kind, owner, label in ((0, 0, 'current-set'), (1 << 17, 0, 'current-mount'),
                            (0, (1 << 64) - 1, 'current-user-owned')):
    request = NamespaceRequest(32, 0, 0, kind, 0, owner)
    identifiers = (ctypes.c_uint64 * 32)()
    count = libc.syscall(470, ctypes.byref(request), identifiers, ctypes.c_size_t(32), 0)
    assert 0 < count < 32, (label, count, ctypes.get_errno())
    actual = set(identifiers[:count])
    assert len(actual) == count
    if label == 'current-set':
        assert actual == set(namespace_ids.values()), (label, actual, namespace_ids)
    elif label == 'current-mount':
        assert actual == {namespace_ids['mnt']}
    else:
        assert actual <= set(namespace_ids.values()) and namespace_ids['mnt'] in actual
print('POLICY_CURRENT_NAMESPACE_QUERY_BOUNDARY_PASS', flush=True)
print('POLICY_NEIGHBOR_NAMESPACE_HANDLES_DENIED_PASS', flush=True)

class MountRequest(ctypes.Structure):
    _fields_ = [('size', ctypes.c_uint32), ('namespace_fd', ctypes.c_uint32),
                ('mount_id', ctypes.c_uint64), ('parameter', ctypes.c_uint64),
                ('namespace_id', ctypes.c_uint64)]

class MountInfoPrefix(ctypes.Structure):
    _fields_ = [('size', ctypes.c_uint32), ('options', ctypes.c_uint32),
                ('mask', ctypes.c_uint64), ('major', ctypes.c_uint32), ('minor', ctypes.c_uint32),
                ('magic', ctypes.c_uint64), ('sbflags', ctypes.c_uint32), ('fstype', ctypes.c_uint32),
                ('mount_id', ctypes.c_uint64), ('parent_id', ctypes.c_uint64),
                ('old_id', ctypes.c_uint32), ('old_parent', ctypes.c_uint32),
                ('attributes', ctypes.c_uint64), ('propagation', ctypes.c_uint64),
                ('peer', ctypes.c_uint64), ('master', ctypes.c_uint64), ('propagate_from', ctypes.c_uint64),
                ('root', ctypes.c_uint32), ('point', ctypes.c_uint32), ('namespace_id', ctypes.c_uint64)]

assert ctypes.sizeof(MountRequest) == 32 and ctypes.sizeof(MountInfoPrefix) == 120
visible_mounts = {int(line.split()[0]) for line in pathlib.Path('/proc/self/mountinfo').read_text().splitlines()}
for namespace_id in (0, namespace_ids['mnt']):
    request = MountRequest(32, 0, (1 << 64) - 1, 0, namespace_id)
    identifiers = (ctypes.c_uint64 * 64)()
    count = libc.syscall(458, ctypes.byref(request), identifiers, ctypes.c_size_t(64), 0)
    assert 0 < count < 64, (count, ctypes.get_errno())
    assert len(set(identifiers[:count])) == count
    old_ids = set()
    for mount_id in identifiers[:count]:
        item = MountRequest(32, 0, mount_id, 2 | 64, namespace_id)
        response = ctypes.create_string_buffer(512)
        assert libc.syscall(457, ctypes.byref(item), response, ctypes.c_size_t(512), 0) == 0, ctypes.get_errno()
        info = MountInfoPrefix.from_buffer_copy(response)
        assert 120 <= info.size <= 512 and info.mask & (2 | 64) == (2 | 64)
        assert info.mount_id == mount_id and info.namespace_id == namespace_ids['mnt']
        old_ids.add(info.old_id)
    assert old_ids == visible_mounts, (old_ids, visible_mounts)
outer_id = int(pathlib.Path('/outer-mount-ns-id').read_text())
assert outer_id != namespace_ids['mnt']
request = MountRequest(32, 0, (1 << 64) - 1, 0, outer_id)
identifiers = (ctypes.c_uint64 * 64)()
ctypes.set_errno(0)
assert libc.syscall(458, ctypes.byref(request), identifiers, ctypes.c_size_t(64), 0) == -1
assert ctypes.get_errno() == errno.ENOENT
assert not any(identifiers)
print('POLICY_OWN_MOUNT_QUERY_MATCHES_PROC_PASS', flush=True)
print('POLICY_OUTER_MOUNT_QUERY_REJECTED_PASS', flush=True)

# Genuine exported handles for owned objects only. No guessed/invented handle
# bytes and no outside namespace or file is targeted.
class ExportedHandle(ctypes.Structure):
    _fields_ = [('bytes', ctypes.c_uint32), ('kind', ctypes.c_int32),
                ('payload', ctypes.c_ubyte * 128)]

def exported_handle(fd):
    handle = ExportedHandle()
    handle.bytes = 128
    mount_id = ctypes.c_int()
    ctypes.set_errno(0)
    assert libc.syscall(303, fd, ctypes.c_char_p(b''), ctypes.byref(handle),
                        ctypes.byref(mount_id), 0x1000) == 0, ctypes.get_errno()
    assert 0 < handle.bytes <= 128
    return handle

own_file = pathlib.Path('/tmp/owned-handle-file')
own_file.write_bytes(b'owned-handle-control')
filefd = os.open(own_file, os.O_RDONLY | os.O_CLOEXEC)
try:
    handle = exported_handle(filefd)
    ctypes.set_errno(0)
    reopened = libc.syscall(304, filefd, ctypes.byref(handle), os.O_RDONLY | os.O_CLOEXEC)
    error = ctypes.get_errno()
    if reopened >= 0:
        os.close(reopened)
    assert reopened == -1 and error == errno.EPERM, ('ordinary file handle', reopened, error)
    assert os.pread(filefd, 64, 0) == b'owned-handle-control'
finally:
    os.close(filefd)
    own_file.unlink()
nsfd = os.open('/proc/self/ns/mnt', os.O_RDONLY | os.O_CLOEXEC)
try:
    handle = exported_handle(nsfd)
    ctypes.set_errno(0)
    reopened = libc.syscall(304, nsfd, ctypes.byref(handle), os.O_RDONLY | os.O_CLOEXEC)
    assert reopened >= 0, ('own namespace handle', ctypes.get_errno())
    try:
        assert os.fstat(reopened).st_ino == os.fstat(nsfd).st_ino
        identifier = bytearray(8)
        assert fcntl.ioctl(reopened, 0x8008b70d, identifier, True) == 0
        assert struct.unpack('=Q', identifier)[0] == namespace_ids['mnt']
        outer_handle = ExportedHandle.from_buffer_copy(pathlib.Path('/outer-mount-handle').read_bytes())
        ctypes.set_errno(0)
        outside = libc.syscall(304, nsfd, ctypes.byref(outer_handle), os.O_RDONLY | os.O_CLOEXEC)
        error = ctypes.get_errno()
        if outside >= 0:
            os.close(outside)
        assert outside == -1 and error == errno.ESTALE, ('outer namespace handle', outside, error)
        print('POLICY_OUTER_NAMESPACE_HANDLE_REOPEN_DENIED_PASS', flush=True)
    finally:
        os.close(reopened)
finally:
    os.close(nsfd)
print('POLICY_OWN_FILE_HANDLE_REOPEN_DENIED_PASS', flush=True)
print('POLICY_CURRENT_NAMESPACE_HANDLE_REOPEN_PASS', flush=True)

# New file-attribute syscalls on one owned tmpfs file. NODUMP is reversible;
# immutable/append changes must be rejected by the existing capability boundary.
class FileAttributes(ctypes.Structure):
    _fields_ = [('xflags', ctypes.c_uint64), ('extsize', ctypes.c_uint32),
                ('nextents', ctypes.c_uint32), ('project', ctypes.c_uint32),
                ('cowextsize', ctypes.c_uint32)]

assert ctypes.sizeof(FileAttributes) == 24
attribute_file = pathlib.Path('/tmp/owned-file-attributes')
attribute_file.write_bytes(b'attribute-control')
attribute_fd = os.open(attribute_file, os.O_RDWR | os.O_CLOEXEC)

def get_attributes():
    value = FileAttributes()
    assert libc.syscall(468, attribute_fd, ctypes.c_char_p(b''), ctypes.byref(value),
                        ctypes.c_size_t(24), 0x1000) == 0, ctypes.get_errno()
    return value

def set_attributes(value):
    ctypes.set_errno(0)
    return libc.syscall(469, attribute_fd, ctypes.c_char_p(b''), ctypes.byref(value),
                        ctypes.c_size_t(24), 0x1000)

original_attributes = get_attributes()
try:
    changed = FileAttributes.from_buffer_copy(original_attributes)
    changed.xflags |= 0x80  # FS_XFLAG_NODUMP
    assert set_attributes(changed) == 0, ctypes.get_errno()
    assert get_attributes().xflags == changed.xflags
    legacy = bytearray(28)  # struct fsxattr
    assert fcntl.ioctl(attribute_fd, 0x801c581f, legacy, True) == 0
    assert struct.unpack_from('=I', legacy)[0] == changed.xflags
    for bit in (8, 16):  # FS_XFLAG_IMMUTABLE / APPEND
        denied = FileAttributes.from_buffer_copy(changed)
        denied.xflags |= bit
        assert set_attributes(denied) == -1 and ctypes.get_errno() == errno.EPERM
        assert get_attributes().xflags == changed.xflags
    assert os.pread(attribute_fd, 64, 0) == b'attribute-control'
finally:
    assert set_attributes(original_attributes) == 0, ctypes.get_errno()
    assert bytes(get_attributes()) == bytes(original_attributes)
    os.close(attribute_fd)
    attribute_file.unlink()
print('POLICY_FILE_ATTRIBUTE_API_PARITY_PASS', flush=True)
print('POLICY_FILE_ATTRIBUTE_PRIVILEGED_FLAGS_DENIED_PASS', flush=True)

# x86_64 fixture: calls use null arguments deliberately. EPERM must precede
# argument validation; none of these requests creates or changes a resource.
assert os.uname().machine == "x86_64"
for name, number in (
    ("mount", 165), ("umount2", 166), ("pivot_root", 155),
    ("fsopen", 430), ("fsconfig", 431), ("fsmount", 432), ("fspick", 433),
    ("open_tree", 428), ("open_tree_attr", 467), ("move_mount", 429), ("mount_setattr", 442),
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
subprocess.run(["/usr/bin/python3", "/process-probe.py", "--restricted"],
               check=True, timeout=5)

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

# Query only independently created private PTYs and this command's own namespace
# objects. These valid calls establish which ioctl interfaces remain reachable.
first_master, first_slave = pty.openpty()
second_master, second_slave = pty.openpty()
try:
    old_second = fcntl.ioctl(second_slave, termios.TIOCGWINSZ, b'\0' * 8)
    fcntl.ioctl(first_slave, termios.TIOCSWINSZ, struct.pack('HHHH', 37, 91, 0, 0))
    assert fcntl.ioctl(second_slave, termios.TIOCGWINSZ, b'\0' * 8) == old_second
    # TIOCGPTPEER returns the slave of this private master, not an unrelated tty.
    peer = fcntl.ioctl(first_master, 0x5441, os.O_RDWR | os.O_NOCTTY | os.O_CLOEXEC)
    try:
        a, b = os.fstat(peer), os.fstat(first_slave)
        assert (a.st_dev, a.st_ino, a.st_rdev) == (b.st_dev, b.st_ino, b.st_rdev)
    finally:
        os.close(peer)
finally:
    for fd in (first_master, first_slave, second_master, second_slave):
        os.close(fd)
own_user = os.stat('/proc/self/ns/user')
for namespace, flag in (('mnt', 0x20000), ('pid', 0x20000000), ('net', 0x40000000),
                        ('ipc', 0x08000000), ('uts', 0x04000000)):
    fd = os.open(f'/proc/self/ns/{namespace}', os.O_RDONLY | os.O_CLOEXEC)
    try:
        assert fcntl.ioctl(fd, 0xb703) == flag  # NS_GET_NSTYPE
        owner = fcntl.ioctl(fd, 0xb701)  # NS_GET_USERNS
        try:
            actual = os.fstat(owner)
            assert (actual.st_dev, actual.st_ino) == (own_user.st_dev, own_user.st_ino)
        finally:
            os.close(owner)
    finally:
        os.close(fd)
print('POLICY_PRIVATE_PTY_PEER_AND_NAMESPACE_IOCTL_PASS', flush=True)

# Generic ioctl paths remain reachable without a device node. Use only owned
# objects: no inherited stdio, supervisor descriptor, or external socket.
def queued(fd):
    return struct.unpack('i', fcntl.ioctl(fd, termios.FIONREAD, bytes(4)))[0]


ioctl_path = pathlib.Path('/tmp/ioctl-owned')
ioctl_path.write_bytes(b'owned-regular')
regular = os.open(ioctl_path, os.O_RDWR | os.O_CLOEXEC)
reader, writer = os.pipe2(os.O_CLOEXEC)
other_reader, other_writer = os.pipe2(os.O_CLOEXEC)
alias = os.dup(reader)
left, right = socket.socketpair()
try:
    assert queued(regular) == len(b'owned-regular')
    os.lseek(regular, 2, os.SEEK_SET)
    assert queued(regular) == len(b'owned-regular') - 2
    assert os.read(regular, 32) == b'ned-regular'
    assert queued(regular) == 0

    os.write(writer, b'owned-pipe')
    assert queued(reader) == queued(alias) == len(b'owned-pipe')
    assert queued(other_reader) == 0
    assert os.read(alias, 32) == b'owned-pipe'
    assert queued(reader) == 0
    # FIONBIO changes the open-file-description status, including a dup alias;
    # it must not change an independently created pipe.
    fcntl.ioctl(reader, termios.FIONBIO, struct.pack('i', 1))
    assert fcntl.fcntl(alias, fcntl.F_GETFL) & os.O_NONBLOCK
    assert not (fcntl.fcntl(other_reader, fcntl.F_GETFL) & os.O_NONBLOCK)
    try:
        os.read(alias, 1)
    except BlockingIOError as error:
        assert error.errno == errno.EAGAIN
    else:
        raise AssertionError('empty nonblocking pipe did not return EAGAIN')
    fcntl.ioctl(alias, termios.FIONBIO, struct.pack('i', 0))
    assert not (fcntl.fcntl(reader, fcntl.F_GETFL) & os.O_NONBLOCK)

    # Close-on-exec is per descriptor, unlike the status flag above.
    fcntl.ioctl(reader, termios.FIONCLEX)
    assert not (fcntl.fcntl(reader, fcntl.F_GETFD) & fcntl.FD_CLOEXEC)
    assert fcntl.fcntl(alias, fcntl.F_GETFD) & fcntl.FD_CLOEXEC
    fcntl.ioctl(reader, termios.FIOCLEX)
    assert fcntl.fcntl(reader, fcntl.F_GETFD) & fcntl.FD_CLOEXEC

    left.sendall(b'owned-socket')
    assert queued(right.fileno()) == len(b'owned-socket')
    assert queued(left.fileno()) == 0
    assert right.recv(32) == b'owned-socket'
    assert queued(right.fileno()) == 0
finally:
    left.close()
    right.close()
    for fd in (regular, reader, writer, alias, other_reader, other_writer):
        os.close(fd)
    ioctl_path.unlink()
print('POLICY_OWNED_FILE_PIPE_SOCKET_IOCTL_PASS', flush=True)


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
