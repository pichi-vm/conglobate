#!/usr/bin/env python3
"""Bounded zstd thread-mode diagnostic using an owned 8MiB buffer."""
import argparse,json,re,shutil,stat,subprocess
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
extras=[];binary=Path(shutil.which('zstd'));extras.append(('bin/zstd',stat.S_IFREG|0o755,binary.read_bytes()))
for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
 extras.append((lib.lstrip('/'),stat.S_IFREG|0o755,Path(lib).read_bytes()))
archive=probe.archive;probe.archive=lambda script:archive(script,extras)
(out/'allowed').write_text('allowed\n');sentinel=out/'secret';sentinel.write_text('secret\n');disk=out/'unused';disk.write_bytes(b'\0'*4096)
script='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
busybox mount -t devtmpfs devtmpfs /dev
echo READY
echo HOSTFS_BOUNDARY_OK
busybox dd if=/dev/urandom of=/tmp/data bs=1M count=8
for mode in --single-thread -T1 -T0; do
 echo "COMPRESSION_START $mode"
 busybox cat /proc/uptime
 zstd -q -f "$mode" -3 /tmp/data -o /tmp/data.zst
 zstd -q -t /tmp/data.zst
 echo "COMPRESSION_END $mode"
 busybox cat /proc/uptime
done
echo COMPLETE
busybox poweroff -f
'''
r=probe.run(out,'compression','run',disk,sentinel,script=script,deadline=30)
(out/'result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
