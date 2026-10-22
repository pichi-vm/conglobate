#!/usr/bin/python3
"""Bounded ordinary writes to an owned 64MiB disposable filesystem."""
import errno
import hashlib
import json
import os
from pathlib import Path
import sys

root = Path(sys.argv[1])
path = root / 'space-fixture'
chunk = b'owned-space-test' * 65536  # 1MiB, with no zero bytes.
accepted = 0
failure = None
fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
try:
    for attempt in range(96):
        try:
            count = os.write(fd, chunk)
            assert 0 < count <= len(chunk)
            accepted += count
        except OSError as error:
            failure = {'operation': 'write', 'errno': errno.errorcode[error.errno]}
            break
        try:
            os.fsync(fd)
        except OSError as error:
            failure = {'operation': 'fsync', 'errno': errno.errorcode[error.errno]}
            break
    assert failure is not None, '96 bounded writes did not reach filesystem limit'
    assert failure['errno'] == 'ENOSPC', failure
finally:
    os.close(fd)
# Verify exactly the successfully accepted prefix, including short writes.
size = path.stat().st_size
content_ok = size == accepted
expected_hash, actual_hash = hashlib.sha256(), hashlib.sha256()
with path.open('rb') as stream:
    offset = 0
    while block := stream.read(len(chunk)):
        # Each write starts the pattern at offset0; this period is retained
        # across full writes. A short write is expected only at the final edge.
        expected = (chunk + chunk)[offset % len(chunk):offset % len(chunk) + len(block)]
        content_ok = content_ok and block == expected
        actual_hash.update(block)
        expected_hash.update(expected)
        offset += len(block)
free_before = os.statvfs(root).f_bfree
path.unlink()
free_after = os.statvfs(root).f_bfree
recovery = root / 'space-recovered'
with recovery.open('xb') as stream:
    stream.write(b'write after freeing space\n')
    stream.flush()
    os.fsync(stream.fileno())
assert recovery.read_bytes() == b'write after freeing space\n'
assert free_after > free_before
result = {'failure': failure, 'accepted': accepted, 'observed_size': size,
          'accepted_content_matches': content_ok, 'sha256': actual_hash.hexdigest(),
          'expected_sha256': expected_hash.hexdigest(),
          'free_blocks_before_unlink': free_before, 'free_blocks_after_unlink': free_after}
print('FILESYSTEM_ENOSPC_OBSERVATION ' + json.dumps(result, sort_keys=True), flush=True)
assert content_ok, 'accepted data did not survive exhaustion'
print('FILESYSTEM_ENOSPC_RECOVERY_PASS', flush=True)
