#!/usr/bin/env python3
"""Safe Rust disabled-policy loading; kernel operations run only in QEMU."""
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
for line in subprocess.check_output(['modprobe', '--show-depends', '--set-version', '7.2.10-200.fc44.x86_64', 'ext4'], text=True).splitlines():
    if line.startswith('insmod '):
        source = pathlib.Path(line.split()[1])
        target = root / source.relative_to('/')
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
subprocess.run(['depmod', '-b', str(root), '7.2.10-200.fc44.x86_64'], check=True)
binary = here / 'target/debug/conglobate-policy-loader-probe'
shutil.copy2(binary, root / 'idmap-probe')
executables = [binary]
for name in ['mke2fs', 'unshare', 'newuidmap', 'newgidmap', 'stat']:
    source = pathlib.Path(shutil.which(name))
    target = root / 'usr/bin' / name
    target.unlink(missing_ok=True)
    shutil.copy2(source, target)
    executables.append(source)
for executable in executables:
    for line in subprocess.check_output(['ldd', str(executable)], text=True).splitlines():
        match = re.search(r'(/\S+)', line)
        if match:
            source = pathlib.Path(match.group(1))
            target = root / source.relative_to('/')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
(root / 'conglobate-disposable-validation-vm').touch()
(root / 'fixture.sh').write_text('''#!/bin/bash
set -eux
export PATH=/usr/bin:/usr/sbin
mkdir -p /work/origin /work/mapped
modprobe loop
modprobe ext4
truncate -s 64M /work/volume.ext4
mke2fs -q -t ext4 -F /work/volume.ext4
mount -o loop /work/volume.ext4 /work/origin
unshare --user --map-users=0:100000:65536 --map-groups=0:100000:65536 sleep 60 &
holder=$!
echo "$holder" > /work/userns-pid
for attempt in {1..50}; do
    if grep -q 100000 /proc/$holder/uid_map; then break; fi
    sleep 0.1
done
cat /proc/$holder/uid_map
cat /proc/$holder/gid_map
mkdir /work/pins
mount -t bpf bpf /work/pins
mkdir /work/cgroups
mount -t cgroup2 cgroup2 /work/cgroups
/idmap-probe
umount /work/cgroups
umount /work/pins
if kill -0 "$holder" 2>/dev/null; then kill "$holder"; fi
wait "$holder" || true
umount /work/origin
echo POLICY_FIXTURE_CLEANUP_PASS
''')
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
bash /fixture.sh
status=$?
echo "IDMAP_PROBE_EXIT $status"
sleep 1
echo b > /proc/sysrq-trigger
exec sleep infinity
''')
paths = sorted(root.rglob('*'))
(out / 'inputs.json').write_text(json.dumps({str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file() and not p.is_symlink()}, indent=2))
archive = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'], input=b'\0'.join(str(p.relative_to(root)).encode() for p in paths) + b'\0', cwd=root, capture_output=True, check=True).stdout
(out / 'initramfs.gz').write_bytes(gzip.compress(archive, compresslevel=1, mtime=0))
shutil.copy2(seed / 'vmlinuz', out / 'vmlinuz')
command = [v.replace(str(seed), str(out)) for v in json.loads((seed / 'command.json').read_text())]
(out / 'command.json').write_text(json.dumps(command, indent=2))
with (out / 'serial.txt').open('wb') as log:
    subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=90, check=True)
serial = (out / 'serial.txt').read_text()
passed = all(marker in serial for marker in ['PRODUCTION_COMMAND_DOMAIN_CANCEL_PASS', 'PRODUCTION_NAMESPACE_PIN_PERSISTENCE_RECOVERY_PASS', 'PRODUCTION_DISABLED_PIN_CLOSE_DROP_PASS', 'PRODUCTION_DISABLED_PIN_RECOVERY_PASS', 'PRODUCTION_POLICY_PERMISSION_FAILURE_PASS', 'PRODUCTION_POLICY_RELOAD_AFTER_FAILURE_PASS', 'PRODUCTION_DISABLED_POLICY_LOAD_DROP_PASS', 'POLICY_FIXTURE_CLEANUP_PASS', 'IDMAP_PROBE_EXIT 0'])
(out / 'verdict.json').write_text(json.dumps({'passed': passed, 'full_sandbox_qualified': False}, indent=2))
assert passed, serial[-5000:]
print('PASS', out)
