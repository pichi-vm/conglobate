#!/usr/bin/python3
from pathlib import Path
mounts = {}
for line in Path('/proc/self/mountinfo').read_text().splitlines():
    fields = line.split(); separator = fields.index('-')
    mounts[fields[4]] = (set(fields[5].split(',')), fields[separator+1])
for root in ('/work/tool', '/work/tool/output'):
    for name, kind, required in (
        ('proc', 'proc', {'nosuid', 'nodev', 'noexec'}),
        ('dev', 'tmpfs', {'nosuid', 'noexec'}),
        ('sys', 'tmpfs', {'ro', 'nosuid', 'nodev', 'noexec'}),
    ):
        flags, actual_kind = mounts[root+'/'+name]
        assert actual_kind == kind and required <= flags
print('DUAL_RUNTIME_FLAGS_AND_TYPES_PASS', flush=True)
