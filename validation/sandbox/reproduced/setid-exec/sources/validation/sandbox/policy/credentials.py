#!/usr/bin/python3
"""Execute only the fixed owned credential reporter after dropping mapped IDs."""
import os
from pathlib import Path
import subprocess
import sys

restricted = sys.argv[1:] == ['--restricted']
assert sys.argv[1:] in ([], ['--restricted'])
reporter = Path('/tmp/setid-observer')
info = reporter.stat()
assert info.st_uid == info.st_gid == 0
assert info.st_mode & 0o7777 == 0o6755
before = Path('/proc/self/status').read_text()
result = subprocess.run([str(reporter)], user=1234, group=1234, extra_groups=[],
                        capture_output=True, text=True, check=True, timeout=3)
assert not result.stderr
fields = result.stdout.split()
assert len(fields) == 7
uid, euid, gid, egid = map(int, fields[:4])
capabilities = int(fields[4], 16)
nnp, seccomp = map(int, fields[5:])
assert uid == gid == 1234
if restricted:
    assert euid == egid == 1234 and capabilities == 0 and nnp == 1 and seccomp == 2, fields
    print('POLICY_SETID_EXEC_NNP_PREVENTS_GAIN_PASS', flush=True)
else:
    assert euid == egid == 0 and capabilities != 0 and nnp == 0, fields
    print('POLICY_SETID_EXEC_POSITIVE_CONTROL_PASS', flush=True)
assert os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 0
# Compare credential/security fields only; process counters legitimately change.
def identity(status):
    return [l for l in status.splitlines() if l.startswith(('Uid:', 'Gid:', 'Cap', 'NoNewPrivs:', 'Seccomp:'))]
assert identity(Path('/proc/self/status').read_text()) == identity(before)
