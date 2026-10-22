#!/usr/bin/python3
"""Supply synthetic inherited handles to the trusted entry fixture."""
import os
import socket
import subprocess
import sys
import errno
import ctypes
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
subprocess.run(["/usr/bin/python3", "/process-probe.py"], check=True, timeout=5)
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
for step in (1, 2):
    subprocess.run(command, check=True, timeout=10, pass_fds=handles)
    print(f"POLICY_STEP_PASS {step}", flush=True)

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
