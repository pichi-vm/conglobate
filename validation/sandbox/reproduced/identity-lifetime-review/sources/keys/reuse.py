#!/usr/bin/python3
"""Owned key canary across worker exit while a trusted observer retains its FD."""
import array
import ctypes
import errno
import os
import socket
import subprocess
import sys

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
KEYCTL, ADD_KEY = 250, 248  # Bounded x86_64 guest fixture.
payload = b'uid-reuse-owned-canary'

def call(number, *args):
    return libc.syscall(ctypes.c_long(number),
        *(ctypes.c_long(a) if isinstance(a, int) else a for a in args))

def session():
    assert call(KEYCTL, 1, ctypes.c_void_p()) > 0

def namespace(uid):
    return ['unshare', '--user', '--mount', '--pid', '--net', '--ipc', '--uts', '--fork',
            '--map-users', f'0:{uid}:65536', '--map-groups', f'0:{uid}:65536',
            '--setuid', '0', '--setgid', '0']

if sys.argv[1:2] == ['worker']:
    session()
    ring = call(KEYCTL, 0, -4, 1)
    assert ring > 0
    # Link per-UID ring into this session to establish possession for ADD_KEY.
    current = call(KEYCTL, 0, -3, 0)
    assert current > 0 and call(KEYCTL, 8, ring, current) == 0
    key = call(ADD_KEY, ctypes.c_char_p(b'user'), ctypes.c_char_p(b'uid-reuse-fixture'),
               ctypes.c_char_p(payload), len(payload), ring)
    assert key > 0 and call(KEYCTL, 5, key, 0x003f0000) == 0
    fd = os.memfd_create('owned-worker-file', os.MFD_CLOEXEC)
    os.write(fd, b'owned-file-canary')
    with socket.socket(fileno=int(sys.argv[2])) as channel:
        channel.sendmsg([str(key).encode()], [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                              array.array('i', [fd]))])
    os.close(fd)
    raise SystemExit(0)

if sys.argv[1:2] in (['read'], ['denied'], ['revoke'], ['revoked']):
    mode, key = sys.argv[1], int(sys.argv[2])
    session()
    if mode == 'revoke':
        assert call(KEYCTL, 3, key) == 0, ctypes.get_errno()
    else:
        buffer = ctypes.create_string_buffer(128)
        result = call(KEYCTL, 11, key, buffer, len(buffer))
        error = ctypes.get_errno()
        if mode == 'read':
            assert result == len(payload) and buffer.raw[:result] == payload
        elif mode == 'denied':
            assert result < 0 and error in (errno.EACCES, errno.EPERM), (result, error)
        else:
            assert result < 0 and error in (errno.EKEYREVOKED, errno.ENOKEY), (result, error)
    raise SystemExit(0)

assert len(sys.argv) == 1 and os.uname().machine == 'x86_64'
left, right = socket.socketpair()
worker = subprocess.Popen(['setpriv', '--reuid=300000', '--regid=300000', '--clear-groups',
                           'python3', '/reuse.py', 'worker', str(right.fileno())],
                          pass_fds=(right.fileno(),), stdin=subprocess.DEVNULL)
right.close()
held = None
try:
    left.settimeout(5)
    message, ancillary, flags, address = left.recvmsg(128, socket.CMSG_SPACE(4))
    assert not flags
    assert len(ancillary) == 1
    level, kind, raw = ancillary[0]
    assert level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS
    handles = array.array('i'); handles.frombytes(raw)
    held, = handles
    key = int(message)
    assert worker.wait(timeout=5) == 0
    assert os.pread(held, 64, 0) == b'owned-file-canary'
    print('UID_REUSE_ORIGINAL_WORKER_REAPED_FD_HELD', flush=True)
    for uid, mode in ((300000, 'read'), (400000, 'denied'),
                      (300000, 'revoke'), (300000, 'revoked')):
        subprocess.run(namespace(uid) + ['python3', '/reuse.py', mode, str(key)],
                       check=True, timeout=5)
        print('UID_REUSE_KEY_OUTCOME', uid, mode, flush=True)
    assert os.pread(held, 64, 0) == b'owned-file-canary'
    print('UID_REUSE_HELD_FD_KEY_LIFETIME_LIMITATION_CONFIRMED', flush=True)
finally:
    left.close()
    if held is not None:
        os.close(held)
    if worker.poll() is None:
        worker.kill()
        worker.wait(timeout=5)
