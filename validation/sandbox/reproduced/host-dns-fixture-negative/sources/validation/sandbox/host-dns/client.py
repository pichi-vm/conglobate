#!/usr/bin/python3
"""Public DNS only; print no resolved addresses."""
import errno
import pathlib
import socket

status = dict(line.split(':', 1) for line in pathlib.Path('/proc/self/status').read_text().splitlines())
assert int(status['NoNewPrivs']) == 1
assert int(status['Seccomp']) == 2
assert socket.getaddrinfo('example.com.', 443, type=socket.SOCK_STREAM)
try:
    with open('/etc/resolv.conf', 'a') as stream:
        stream.write('must fail\n')
except OSError as error:
    assert error.errno == errno.EROFS
else:
    raise AssertionError('resolver injection is writable')
print('HOST_PUBLIC_DNS_RESTRICTED_PASS', flush=True)
