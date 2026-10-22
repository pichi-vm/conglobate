#!/usr/bin/python3
"""Supply synthetic inherited handles to the trusted entry fixture."""
import os
import socket
import subprocess
import sys
import errno
import ctypes
import fcntl
import struct
import json
import select
import signal
import time
from pathlib import Path

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long

# Namespace-only negative control, using solely disposable fixture objects.
if sys.argv[1:] == ["--ipc-handles"]:
    key = int(Path("/work/root/known-key").read_text())
    buf = ctypes.create_string_buffer(64)
    size = libc.syscall(250, 11, key, buf, 64)
    assert size == len(b"policy-key-canary") and buf.raw[:size] == b"policy-key-canary"
    print("NAMESPACE_ONLY_REAL_KEY_READ_PASS", flush=True)
    assert os.pread(271, 64, 0) == b"memfd-canary"
    assert os.pread(272, 64, 0) == b"shm-canary"
    assert os.eventfd_read(273) == 7
    os.eventfd_write(273, 7)
    peer = socket.socket(fileno=274)
    peer.settimeout(1)
    assert peer.recv(64) == b"socketpair-canary"
    peer.sendall(b"socketpair-reply")
    pathname = socket.socket(socket.AF_UNIX)
    pathname.settimeout(1)
    pathname.connect("/supervisor.sock")
    pathname.sendall(b"pathname-canary")
    abstract = socket.socket(socket.AF_UNIX)
    abstract.settimeout(1)
    try:
        abstract.connect("\0conglobate-ipc-fixture")
    except OSError as error:
        assert error.errno == errno.ECONNREFUSED, error
    else:
        raise AssertionError("abstract socket crossed network namespace")
    print("NAMESPACE_ONLY_INHERITED_IPC_AND_PATH_VISIBLE", flush=True)
    print("PRIVATE_NETWORK_ABSTRACT_SOCKET_HIDDEN", flush=True)
    sys.exit(0)

assert os.uname().machine == "x86_64"
# Identify only this owned guest fixture's outer mount namespace for a
# read-only visibility control; no host namespace or path is involved.
outer_namespace = os.open('/proc/self/ns/mnt', os.O_RDONLY | os.O_CLOEXEC)
try:
    identifier = bytearray(8)
    assert fcntl.ioctl(outer_namespace, 0x8008b70d, identifier, True) == 0
    Path('/work/root/outer-mount-ns-id').write_text(str(struct.unpack('=Q', identifier)[0]))
    class OuterHandle(ctypes.Structure):
        _fields_ = [('bytes', ctypes.c_uint32), ('kind', ctypes.c_int32),
                    ('payload', ctypes.c_ubyte * 128)]
    handle = OuterHandle()
    handle.bytes = 128
    mount_id = ctypes.c_int()
    assert libc.syscall(303, outer_namespace, ctypes.c_char_p(b''), ctypes.byref(handle),
                        ctypes.byref(mount_id), 0x1000) == 0, ctypes.get_errno()
    assert 0 < handle.bytes <= 128
    # A genuine kernel-produced handle for this owned guest fixture, not a
    # guessed identifier or a handle to any host object.
    Path('/work/root/outer-mount-handle').write_bytes(bytes(handle))
    positive = libc.syscall(304, outer_namespace, ctypes.byref(handle), os.O_RDONLY | os.O_CLOEXEC)
    assert positive >= 0, ('outer handle before', ctypes.get_errno())
    assert os.fstat(positive).st_ino == os.fstat(outer_namespace).st_ino
    os.close(positive)
    print('POLICY_OUTER_HANDLE_VALID_BEFORE', flush=True)

finally:
    outer_control = os.dup(outer_namespace)
    os.close(outer_namespace)
# Read-only acquisition of an owned fixture mount's reconfiguration context.
# Close it without issuing fsconfig or altering the filesystem.
fspick = libc.syscall(433, -100, ctypes.c_char_p(b'/work'), 1)
assert fspick >= 0, ('unrestricted fspick control', ctypes.get_errno())
os.close(fspick)
print('POLICY_FSPICK_OWNED_CONTROL_PASS', flush=True)
subprocess.run(["/usr/bin/python3", "/process-probe.py"], check=True, timeout=5)
subprocess.run(["/usr/bin/python3", "/cross-process.py"], check=True, timeout=15)
subprocess.run(["/usr/bin/python3", "/worker-identity.py"], check=True, timeout=20)
ring = libc.syscall(250, 1, ctypes.c_void_p())
assert ring > 0
payload = b"policy-key-canary"
key = libc.syscall(248, ctypes.c_char_p(b"user"), ctypes.c_char_p(b"policy-canary"),
                   ctypes.c_char_p(payload), len(payload), ring)
assert key > 0
assert libc.syscall(250, 5, key, 0x3f3f0000) == 0
Path("/work/root/known-key").write_text(str(key))
tcp = socket.socket()
tcp.bind(("127.0.0.1", 0))
tcp.listen(1)
unix = socket.socket(socket.AF_UNIX)
unix.bind("/supervisor.sock")
unix.listen(1)
os.chmod("/supervisor.sock", 0o666)
abstract = socket.socket(socket.AF_UNIX)
abstract.bind("\0conglobate-ipc-fixture")
abstract.listen(1)
check = socket.socket(socket.AF_UNIX)
check.connect("\0conglobate-ipc-fixture")
accepted, _ = abstract.accept()
accepted.close()
check.close()
print("SUPERVISOR_ABSTRACT_SOCKET_POSITIVE_CONTROL", flush=True)
memfd = os.memfd_create("disposable-canary")
os.write(memfd, b"memfd-canary")
os.makedirs("/dev/shm", exist_ok=True)
shm = os.open("/dev/shm/supervisor-canary", os.O_CREAT | os.O_EXCL | os.O_RDWR, 0o600)
os.write(shm, b"shm-canary")
event = os.eventfd(7)
pair, other = socket.socketpair()
other.settimeout(1)
for source, target in ((memfd, 271), (shm, 272), (event, 273), (pair.fileno(), 274)):
    os.dup2(source, target)
other.sendall(b"socketpair-canary")
read_fd, write_fd = os.pipe()
for source, target in ((tcp.fileno(), 258), (unix.fileno(), 259), (read_fd, 270)):
    os.dup2(source, target)
os.write(write_fd, b"synthetic-pipe-canary\n")
command = [
    "unshare", "--user", "--mount", "--pid", "--net", "--ipc", "--uts", "--fork",
    "--map-users", "0:100000:65536", "--map-groups", "0:100000:65536",
    "--setuid", "0", "--setgid", "0", "/bin/bash", "/launch",
]
subprocess.run(command[:-1] + ["/credential-launch"], check=True, timeout=10)
before_mounts = Path("/proc/self/mountinfo").read_text()
before_dev = sorted(p.name for p in Path("/work/root/dev").iterdir())
handles = (9, 257, 258, 259, 270, 271, 272, 273, 274)
subprocess.run(command[:-2] + ["/usr/bin/python3", "/driver.py", "--ipc-handles"],
               check=True, timeout=10, pass_fds=handles)
assert other.recv(64) == b"socketpair-reply"
accepted, _ = unix.accept()
accepted.settimeout(1)
assert accepted.recv(64) == b"pathname-canary"
accepted.close()
class CollectorOutputLimit(RuntimeError):
    pass

class CollectorDeadline(TimeoutError):
    pass

def collect_owned_output(process, read_log, seconds=10, limit=1024*1024):
    output = bytearray()
    deadline = time.monotonic() + seconds
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise CollectorDeadline(bytes(output))
            if not select.select([read_log], [], [], min(remaining, .1))[0]:
                continue
            chunk = os.read(read_log, 65536)
            if not chunk:
                break
            output.extend(chunk)
            if len(output) > limit:
                raise CollectorOutputLimit(len(output))
        assert process.wait(timeout=max(.01, deadline-time.monotonic())) == 0
        return output
    finally:
        # This process group belongs only to this disposable probe launch.
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=2)
        os.close(read_log)

for step in (1, 2):
    # Owned standard streams only: never expose the guest console to the probe.
    with open('/work/runtime/null', 'rb', buffering=0) as source:
        read_log, write_log = os.pipe()
        identity = lambda fd: [os.fstat(fd).st_dev, os.fstat(fd).st_ino]
        Path('/work/root/stdio-identity').write_text(json.dumps(
            {'stdin': identity(source.fileno()), 'output': identity(write_log)}))
        try:
            process = subprocess.Popen(command, stdin=source, stdout=write_log,
                                       stderr=write_log, pass_fds=handles,
                                       start_new_session=True)
        except BaseException:
            os.close(read_log)
            raise
        finally:
            os.close(write_log)
        output = collect_owned_output(process, read_log)
        lines = output.decode().splitlines()
        for marker in ('POLICY_OWNED_STDIO_PASS', 'OWNED_STDOUT_DUP', 'OWNED_STDERR_DUP'):
            assert lines.count(marker) == 1, (marker, lines)
        sys.stdout.write(output.decode())
        print(f'POLICY_OWNED_STDIO_COLLECTOR_EOF_PASS {step}', flush=True)
    print(f"POLICY_STEP_PASS {step}", flush=True)

# Bounded ordinary-output fixtures run through the same restricted launcher.
# Neither writes to outside paths nor receives a trusted console.
probe_path = Path('/work/root/probe')
original_probe = probe_path.read_bytes()
try:
    for mode, body, expected in (
        ('limit', "os.write(1, b'x'*(2*1024*1024)); time.sleep(10)", CollectorOutputLimit),
        ('held-pipe', "os.write(1, b'OWNED_PIPE_OPEN\\n'); os.fork(); time.sleep(10)", CollectorDeadline),
        ('eof-live', "os.close(1); os.close(2); time.sleep(10)", subprocess.TimeoutExpired),
    ):
        probe_path.write_text('#!/usr/bin/python3\nimport os,time\n'+body+'\n')
        baseline = set(os.listdir('/proc/self/fd'))
        with open('/work/runtime/null', 'rb', buffering=0) as source:
            read_log, write_log = os.pipe()
            observer = os.dup(read_log)
            try:
                process = subprocess.Popen(command, stdin=source, stdout=write_log,
                                           stderr=write_log, start_new_session=True)
            finally:
                os.close(write_log)
            began = time.monotonic()
            try:
                try:
                    collect_owned_output(process, read_log, seconds=3)
                except expected as error:
                    if mode == 'limit':
                        assert 1024*1024 < error.args[0] <= 1024*1024+65536
                    elif mode == 'held-pipe':
                        assert error.args[0] == b'OWNED_PIPE_OPEN\n'
                else:
                    raise AssertionError('collector accepted bounded failure '+mode)
                assert process.returncode is not None
                assert time.monotonic()-began < 6
                # A remaining pipe writer would prevent EOF. Drain only this
                # owned pipe, bounded by its queued bytes and a short deadline.
                deadline = time.monotonic()+2
                drained = 0
                while True:
                    assert time.monotonic()<deadline
                    if not select.select([observer], [], [], .05)[0]: continue
                    chunk=os.read(observer,65536)
                    if not chunk: break
                    drained+=len(chunk); assert drained<=2*1024*1024
            finally:
                os.close(observer)
        assert set(os.listdir('/proc/self/fd')) == baseline
        print('POLICY_COLLECTOR_FAILURE_CLEANUP_PASS '+mode, flush=True)
finally:
    probe_path.write_bytes(original_probe)

assert os.pread(memfd, 64, 0) == b"memfd-canary"
assert os.pread(shm, 64, 0) == b"shm-canary"
assert os.eventfd_read(event) == 7
other.sendall(b"still-live")
pair.settimeout(1)
assert pair.recv(64) == b"still-live"
print("SUPERVISOR_IPC_HANDLES_UNCHANGED_PASS", flush=True)

buf = ctypes.create_string_buffer(64)
size = libc.syscall(250, 11, key, buf, 64)
assert size == len(payload) and buf.raw[:size] == payload
assert libc.syscall(250, 3, key) == 0
print("SUPERVISOR_KEY_UNCHANGED_PASS", flush=True)

assert Path("/proc/self/mountinfo").read_text() == before_mounts
assert sorted(p.name for p in Path("/work/root/dev").iterdir()) == before_dev
print("POLICY_SUPERVISOR_MOUNTS_AND_RUNTIME_BACKING_UNCHANGED_PASS", flush=True)

positive = libc.syscall(304, outer_control, ctypes.byref(handle), os.O_RDONLY | os.O_CLOEXEC)
assert positive >= 0, ('outer handle after', ctypes.get_errno())
assert os.fstat(positive).st_ino == os.fstat(outer_control).st_ino
os.close(positive)
os.close(outer_control)
print('POLICY_OUTER_HANDLE_VALID_AFTER', flush=True)
