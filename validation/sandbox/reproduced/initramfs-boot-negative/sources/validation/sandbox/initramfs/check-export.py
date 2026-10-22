#!/usr/bin/env python3
"""Bounded synthetic framing tests; never start QEMU with invalid exports."""
import hashlib
from boot import decode, LIMIT


def frame(parts):
    return b'CONGLOBATE_BOOT_FIXTURE_V1\n' + b''.join(
        str(len(p)).encode() + b' ' + hashlib.sha256(p).hexdigest().encode() + b'\n' + p
        for p in parts)

valid = frame([b'fixture kernel', b'fixture initramfs'])
assert decode(valid) == {'kernel': b'fixture kernel', 'initramfs': b'fixture initramfs'}
cases = [b'', valid[:-1], valid + b'extra', valid.replace(b'fixture kernel', b'changed kernel'),
         valid.replace(b'14 ', b'0 ', 1),
         b'CONGLOBATE_BOOT_FIXTURE_V1\n' + str(LIMIT + 1).encode() + b' ' + b'0' * 64 + b'\n']
for data in cases:
    try:
        decode(data)
    except ValueError:
        pass
    else:
        raise AssertionError('invalid export accepted')
print('EXPORT_VALIDATION_PASS: roundtrip; magic/truncation/trailing/digest/zero-size/oversize rejection')
