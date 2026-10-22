#!/usr/bin/python3
"""Trusted bounded diagnostic export after writer exit and snapshot sealing."""
from pathlib import Path
import hashlib
import json
root=Path('/work/export')
entries=[]
for name in ('data','hash','root'):
    p=root/name
    assert 0<p.stat().st_size<=128*1024*1024
    with p.open('rb') as f: digest=hashlib.file_digest(f,'sha256').hexdigest()
    entries.append({'name':name,'bytes':p.stat().st_size,'sha256':digest})
with open('/dev/vport0p1','wb',buffering=0) as output:
    output.write((json.dumps(entries)+'\n').encode())
    for entry in entries:
        with (root/entry['name']).open('rb') as source:
            while chunk:=source.read(1024*1024):
                view=memoryview(chunk)
                while view:
                    count=output.write(view)
                    assert count>0
                    view=view[count:]
print('DERIVED_BOOT_SERIAL_EXPORT_PASS',flush=True)
