#!/usr/bin/env python3
"""Run only the benign checkout-mount probe in a disposable QEMU guest."""
import gzip
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys

here = pathlib.Path(__file__).resolve().parent
seed = pathlib.Path('/var/tmp/conglobate-cap-review-fix-normal-v1')
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir()
root = out / 'root'
shutil.copytree(seed / 'root', root, symlinks=True)
binary = here / 'target/debug/rewrite-context-mount-probe'
shutil.copy2(binary, root / 'context-probe')
for line in subprocess.check_output(['ldd', str(binary)], text=True).splitlines():
    match = re.search(r'(/\S+)', line)
    if match:
        source = pathlib.Path(match.group(1))
        destination = root / source.relative_to('/')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
fixture = root / 'fixture'
fixture.mkdir()
subprocess.run(['git', 'init', '-q', str(fixture)], check=True)
(fixture / 'file').write_bytes(b'committed fixture')
subprocess.run(['git', '-C', str(fixture), 'add', 'file'], check=True)
subprocess.run(['git', '-C', str(fixture), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture'], check=True)
(root / 'fixture-commit').write_bytes(subprocess.check_output(['git', '-C', str(fixture), 'rev-parse', 'HEAD']))
(root / 'conglobate-disposable-validation-vm').touch()
(root / 'init').write_text('''#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
mkdir -p /work
mount -t tmpfs tmpfs /work
set +e
/context-probe
status=$?
echo "CONTEXT_PROBE_EXIT $status"
sleep 1
echo b > /proc/sysrq-trigger
exec sleep infinity
''')
paths = sorted(root.rglob('*'))
(out / 'inputs.json').write_text(json.dumps({str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file() and not p.is_symlink()}, indent=2))
archive = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'], input=b'\0'.join(str(p.relative_to(root)).encode() for p in paths) + b'\0', cwd=root, capture_output=True, check=True).stdout
(out / 'initramfs.gz').write_bytes(gzip.compress(archive, compresslevel=1, mtime=0))
shutil.copy2(seed / 'vmlinuz', out / 'vmlinuz')
command = [value.replace(str(seed), str(out)) for value in json.loads((seed / 'command.json').read_text())]
(out / 'command.json').write_text(json.dumps(command, indent=2))
with (out / 'serial.txt').open('wb') as log:
    subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=90, check=True)
serial = (out / 'serial.txt').read_text()
assert 'PRODUCTION_CONTEXT_PARTIAL_SETUP_PASS' in serial and 'PRODUCTION_CONTEXT_LIFECYCLE_PASS' in serial and 'READONLY_CONTEXT_VIEW_PASS' in serial and 'CONTEXT_PROBE_EXIT 0' in serial, serial[-4000:]
(out / 'verdict.json').write_text(json.dumps({'readonly_view': True, 'pre_mount_fd_is_writable': True, 'production_partial_setup_cleanup': True, 'production_checkout_lifecycle': True, 'full_sandbox_qualified': False}, indent=2))
print('PASS', out)
