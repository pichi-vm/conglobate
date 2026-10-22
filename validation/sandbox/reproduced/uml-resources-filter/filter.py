import os,errno,socket
from pathlib import Path
s=Path('/proc/self/status').read_text()
assert 'Seccomp:	2' in s
try:os.unshare(os.CLONE_NEWUSER)
except OSError as e:assert e.errno==errno.EPERM
else:raise AssertionError('user namespace creation allowed')
try:socket.socket(socket.AF_VSOCK)
except OSError as e:assert e.errno==errno.EPERM
else:raise AssertionError('VSOCK allowed')
print('FILTER_OK')
