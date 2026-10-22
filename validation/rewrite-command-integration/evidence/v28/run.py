#!/usr/bin/env python3
"""Integrated command fixture; kernel operations run only in disposable QEMU."""
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
supervisor_death = len(sys.argv) == 3 and sys.argv[2] == '--supervisor-death'
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
binary = here / 'target/debug/conglobate-command-integration'
shutil.copy2(binary, root / 'idmap-probe')
executables = [binary]
for name in ['mke2fs', 'unshare', 'newuidmap', 'newgidmap', 'stat', 'bwrap', 'nsenter', 'e2fsck', 'rmdir', 'setpriv']:
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
image_root = root / 'image-root'
(image_root / 'bin').mkdir(parents=True)
for source, name in [('/usr/bin/bash', 'sh')]:
    shutil.copy2(source, image_root / 'bin' / name)
    for line in subprocess.check_output(['ldd', source], text=True).splitlines():
        match = re.search(r'(/\S+)', line)
        if match:
            library = pathlib.Path(match.group(1))
            target = image_root / library.relative_to('/')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(library, target)
for name in ['proc', 'dev', 'sys']:
    (image_root / name).mkdir()
(root / 'empty-sys').mkdir(exist_ok=True)
(root / 'supervisor-secret').write_text('unchanged')
(root / 'fixture.sh').write_text('''#!/bin/bash
set -eux
export PATH=/usr/bin:/usr/sbin
mkdir -p /work/origin /work/mapped
unshare --user --map-root-user /idmap-probe privileges
unshare --user --map-root-user /idmap-probe syscalls
if unshare --user --map-root-user setpriv --bounding-set=-chown /idmap-probe privileges > /work/privileges-negative 2>&1; then
    echo UNEXPECTED_PRIVILEGE_FINALIZATION_SUCCESS
    exit 1
fi
grep -q 'required capability is not available' /work/privileges-negative
if grep -q NATIVE_PRIVILEGES_EXEC_PASS /work/privileges-negative; then exit 1; fi
echo NATIVE_PRIVILEGES_MISSING_CAP_REJECTED
modprobe loop
modprobe ext4
truncate -s 64M /work/volume.ext4
mke2fs -q -t ext4 -F /work/volume.ext4
mount -o loop /work/volume.ext4 /work/origin
cp -a /image-root/. /work/origin/
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
/idmap-probe supervisor success
rm /work/origin/command-started
/idmap-probe supervisor timeout
umount /work/cgroups
umount /work/pins
if kill -0 "$holder" 2>/dev/null; then kill "$holder"; fi
wait "$holder" || true
umount /work/origin
e2fsck -fn /work/volume.ext4
mount -o loop,ro /work/volume.ext4 /work/origin
test ! -e /work/origin/dev/null
test ! -e /work/origin/dev/fd
test ! -e /work/origin/sys/must-be-read-only
test "$(cat /work/origin/command-completed)" = completed
test "$(cat /work/origin/command-privileges)" = privileges-checked
test "$(stat -c %u /work/origin/command-started)" = 0
umount /work/origin
echo INTEGRATED_REMOUNT_PERSISTENCE_PASS
# Diagnose and remove only empty fixture mount-point directories. This is not
# production crash recovery; persistent target bookkeeping remains required.
for point in /work/conglobate-mount-*; do
    test -d "$point" || continue
    echo INTEGRATION_EMPTY_TARGET_REMAINS
    rmdir "$point"
done
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
if supervisor_death:
    fixture = root / 'fixture.sh'
    setup = fixture.read_text().split('/idmap-probe supervisor success')[0]
    fixture.write_text(setup + r"""
/idmap-probe supervisor vm-death &
supervisor=$!
for attempt in {1..100}; do
    test ! -e /work/supervisor-ready || break
    sleep 0.05
done
test -e /work/supervisor-ready
kill -KILL "$supervisor"
set +e
wait "$supervisor"
status=$?
set -e
test "$status" = 137
# The independent controller has observed supervisor loss. Confirm there are
# still contained processes: shutdown must not depend on supervisor cleanup.
populated=0
for domain in /work/cgroups/conglobate-*; do
    if grep -q 'populated 1' "$domain/cgroup.events"; then populated=1; fi
done
test "$populated" = 1
echo SUPERVISOR_DEATH_OBSERVED_WITH_LIVE_DOMAIN
# Return to PID 1, which powers off this VM. Never start another build here.
""")
    init = root / 'init'
    init.write_text(init.read_text().replace('sleep 1\necho b >',
        'echo VM_CONTROLLER_POWER_OFF\necho o >'))
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
if supervisor_death:
    # Kernel console output can interrupt a userspace status line during power
    # off. Remove complete timestamped kernel messages only for status parsing;
    # still require the actual kernel shutdown message in the untouched log.
    userspace = re.sub(r'\[\s*\d+\.\d+\] [^\n]*\n', '', serial)
    passed = all(marker in userspace for marker in [
        'SUPERVISOR_DEATH_OBSERVED_WITH_LIVE_DOMAIN', 'IDMAP_PROBE_EXIT 0',
        'VM_CONTROLLER_POWER_OFF',
    ]) and 'Power down' in serial and 'INTEGRATED_COMMAND_' not in serial
else:
    passed = all(marker in serial for marker in ['NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS', 'NATIVE_PRIVILEGES_MISSING_CAP_REJECTED', 'NATIVE_PRIVILEGES_EXEC_PASS', 'INTEGRATED_COMMAND_SUCCESS_PASS', 'INTEGRATED_COMMAND_TIMEOUT_PASS', 'INTEGRATED_REMOUNT_PERSISTENCE_PASS', 'POLICY_FIXTURE_CLEANUP_PASS', 'IDMAP_PROBE_EXIT 0'])
(out / 'verdict.json').write_text(json.dumps({'passed': passed, 'full_sandbox_qualified': False}, indent=2))
assert passed, serial[-5000:]
print('PASS', out)
