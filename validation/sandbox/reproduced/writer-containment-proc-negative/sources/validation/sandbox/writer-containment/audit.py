#!/usr/bin/python3
"""Read-only descriptor metadata audit inside an owned disposable guest."""
import json
import os
from pathlib import Path
import sys

mode, ancestor = sys.argv[1], int(sys.argv[2])
assert mode in ('before', 'after')


def contained(pid):
    seen = set()
    while pid > 0 and pid not in seen:
        if pid == ancestor:
            return True
        seen.add(pid)
        fields = dict(line.split(':', 1) for line in Path(f'/proc/{pid}/status').read_text().splitlines())
        pid = int(fields['PPid'])
    return False


rows = []
processes = [p for p in Path('/proc').iterdir() if p.name.isdecimal()]
assert len(processes) < 128, 'bounded guest process inventory'
for process in processes:
    try:
        mounts = {}
        for line in (process / 'mountinfo').read_text().splitlines():
            left, right = line.split(' - ', 1)
            mounts[int(left.split()[0])] = right.split()[0]
        for fd in (process / 'fdinfo').iterdir():
            try:
                info = dict(line.split(':', 1) for line in fd.read_text().splitlines() if ':' in line)
                flags = int(info['flags'].strip(), 8)
                if flags & os.O_ACCMODE == os.O_RDONLY:
                    continue
                target = os.readlink(process / 'fd' / fd.name)
                filesystem = mounts.get(int(info['mnt_id']), '')
                if target.startswith(('/dev/dm-', '/dev/loop')) and target != '/dev/loop-control':
                    kind = 'block'
                elif target == '/dev/fuse':
                    kind = 'fuse-device'
                elif filesystem.startswith('fuse'):
                    kind = 'image-file'
                elif target.startswith('/tmp/conglobate-') and target.endswith('/data'):
                    kind = 'cow-file'
                else:
                    continue
                pid = int(process.name)
                assert mode == 'before', ('writer handle survived namespace init', pid, target)
                assert contained(pid), ('writable handle outside lifetime namespace', pid, target)
                rows.append({'pid': pid, 'fd': int(fd.name), 'kind': kind, 'target': target,
                             'filesystem': filesystem, 'flags': oct(flags)})
            except FileNotFoundError:
                # Short-lived unrelated shell helpers may disappear. Required
                # persistent worker/command handles have explicit count checks.
                continue
    except FileNotFoundError:
        continue
if mode == 'before':
    assert sum(r['kind'] == 'block' for r in rows) >= 2, rows
    assert sum(r['kind'] == 'fuse-device' for r in rows) >= 1, rows
    assert sum(r['kind'] == 'image-file' for r in rows) >= 2, rows
else:
    assert not rows
print('WRITER_CONTAINMENT_FD_INVENTORY', mode, json.dumps(rows, sort_keys=True), flush=True)
print('WRITER_CONTAINMENT_AUDIT_PASS', mode, flush=True)
