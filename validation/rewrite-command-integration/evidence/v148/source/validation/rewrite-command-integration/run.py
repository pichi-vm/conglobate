#!/usr/bin/env python3
"""Integrated command fixture; kernel operations run only in disposable QEMU."""
import gzip
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
from cache_fixture import CacheFixture

here = pathlib.Path(__file__).resolve().parent
seed = pathlib.Path('/var/tmp/conglobate-cap-review-fix-normal-v1')
supervisor_death = len(sys.argv) == 3 and sys.argv[2] == '--supervisor-death'
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir()
# Never qualify a preceding binary after a failed build. Retain compiler output
# with this run and abort before constructing or starting the guest on failure.
build_env = os.environ.copy()
build_env.setdefault('TMPDIR', '/var/tmp/conglobate-rewrite-tmp')
with (out / 'build.txt').open('wb') as build_log:
    subprocess.run(['cargo', 'build', '--locked', '--manifest-path', str(here / 'Cargo.toml')],
                   env=build_env, stdout=build_log, stderr=subprocess.STDOUT, check=True)
root = out / 'root'
shutil.copytree(seed / 'root', root, symlinks=True)
for module in ['ext4', 'dm-zero', 'dm-snapshot', 'dm-verity']:
    for line in subprocess.check_output(['modprobe', '--show-depends', '--set-version', '7.2.10-200.fc44.x86_64', module], text=True).splitlines():
        if line.startswith('insmod '):
            source = pathlib.Path(line.split()[1])
            target = root / source.relative_to('/')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
subprocess.run(['depmod', '-b', str(root), '7.2.10-200.fc44.x86_64'], check=True)
binary = here / 'target/debug/conglobate-command-integration'
shutil.copy2(binary, root / 'idmap-probe')
executables = [binary]
for name in ['mke2fs', 'unshare', 'newuidmap', 'newgidmap', 'stat', 'e2fsck', 'rmdir', 'setpriv', 'getfattr']:
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
for name in ['nsenter', 'bwrap']:
    (root / 'usr/bin' / name).unlink(missing_ok=True)
# A committed, local input repository for production context materialization.
context_fixture = root / 'workspace-context'
context_fixture.mkdir()
(context_fixture / 'input').write_text('committed context\n')
subprocess.run(['git', 'init', '--quiet', str(context_fixture)], check=True)
subprocess.run(['git', '-C', str(context_fixture), 'add', 'input'], check=True)
subprocess.run(['git', '-C', str(context_fixture), '-c', 'user.name=Fixture',
                '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false',
                'commit', '--quiet', '-m', 'Fixture'], check=True)
(root / 'workspace-context-commit').write_text(subprocess.check_output(
    ['git', '-C', str(context_fixture), 'rev-parse', 'HEAD'], text=True))
(root / 'conglobate-disposable-validation-vm').touch()
CacheFixture(root / 'cache-fixture').create()
image_root = root / 'image-root'
(image_root / 'bin').mkdir(parents=True)
for source, name in [('/usr/bin/bash', 'sh'), ('/usr/bin/setfattr', 'setfattr'), ('/usr/bin/getfattr', 'getfattr'), ('/usr/bin/mv', 'mv')]:
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
mkdir -p /work/origin /work/mapped /work/conglobate-mount-decoy
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
/idmap-probe namespace-rejections
modprobe dm-zero
modprobe dm-snapshot
/idmap-probe snapshot-probe
modprobe dm-verity
/idmap-probe prepare-scutes
/idmap-probe assemble-chain
mkdir /work/cgroups
mount -t cgroup2 cgroup2 /work/cgroups
mkdir /work/pins
mount -t bpf bpf /work/pins
chmod 700 /work/pins
/idmap-probe domain-publication-probe
if /idmap-probe volume-supervisor-sandbox > /work/command-diagnostics 2>&1; then
  cat /work/command-diagnostics
else
  cat /work/command-diagnostics
  exit 1
fi
/idmap-probe volume-supervisor-sandbox-namespace-failure
/idmap-probe volume-supervisor-sandbox-mount-failure
/idmap-probe volume-probe
/idmap-probe volume-supervisor
/idmap-probe volume-supervisor-launch-failure
/idmap-probe volume-supervisor-exchange-failure
/idmap-probe volume-supervisor-registration-failure
/idmap-probe volume-supervisor-execute-success
/idmap-probe volume-supervisor-execute-failure
/idmap-probe volume-supervisor-execute-timeout
/idmap-probe volume-supervisor-execute-setup
/idmap-probe volume-supervisor-domain-failure
/idmap-probe volume-supervisor-install-failure
/idmap-probe volume-supervisor-timeout
/idmap-probe volume-supervisor-failure
/idmap-probe volume-supervisor-pre-admission
/idmap-probe volume-supervisor-admission-timeout
/idmap-probe volume-supervisor-setup-detached
/idmap-probe volume-supervisor-setup-attached
/idmap-probe volume-supervisor-policy-cleanup
/idmap-probe volume-supervisor-long-command
/idmap-probe volume-supervisor-descriptor-rejection
/idmap-probe loop-probe
/idmap-probe mapping-probe
/idmap-probe workspace-probe
/idmap-probe reaper-probe
/idmap-probe filesystem-pin-probe
# Preflight probes are ready. User namespace and command setup must be native.
rm /usr/bin/unshare
test ! -e /usr/bin/nsenter
test ! -e /usr/bin/bwrap
/idmap-probe supervisor pre-admission-cancel
/idmap-probe supervisor admission-timeout
/idmap-probe supervisor policy-off
rm /work/origin/command-started
/idmap-probe supervisor success
rm /work/origin/command-started
/idmap-probe supervisor timeout
rm /work/origin/command-started
/idmap-probe supervisor shell-failure
umount /work/cgroups
umount /work/pins
umount /work/origin
e2fsck -fn /work/volume.ext4
mount -o loop,ro /work/volume.ext4 /work/origin
test "$(getfattr --only-values -n security.selinux /work/origin/labelled)" = 'system_u:object_r:etc_t:s0'
echo INTEGRATED_LABEL_REMOUNT_PERSISTENCE_PASS
test ! -e /work/origin/dev/null
test ! -e /work/origin/dev/fd
test ! -e /work/origin/dev/pts
test ! -e /work/origin/dev/shm
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
    setup = fixture.read_text().split('/idmap-probe supervisor policy-off')[0]
    fixture.write_text(setup + r"""
/idmap-probe volume-supervisor-vm-death &
supervisor=$!
/idmap-probe wait-marker /work/supervisor-ready
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
# Verify the production filesystem pin remains attached after supervisor death.
pin=
volume=
while read -r mount_id parent_id device mount_root mount_path rest; do
    case "$mount_path" in
        /work/volume-policy-records/filesystem-*) test -z "$pin"; pin="$mount_path";;
        /work/conglobate-volume-*) test -z "$volume"; volume="$mount_path";;
    esac
done < /proc/self/mountinfo
test -n "$pin"
test -n "$volume"
umount "$volume"
touch "$pin/supervisor-gone"
/idmap-probe wait-marker "$pin/death-policy-checked"
test "$(cat "$pin/death-policy-checked")" = checked
echo OWNED_VOLUME_AFTER_SUPERVISOR_DEATH_PASS
echo FILESYSTEM_PIN_AFTER_SUPERVISOR_DEATH_PASS
echo ACTIVE_POLICY_AFTER_SUPERVISOR_DEATH_PASS
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
        'VM_CONTROLLER_POWER_OFF', 'ACTIVE_POLICY_AFTER_SUPERVISOR_DEATH_PASS',
        'FILESYSTEM_PIN_AFTER_SUPERVISOR_DEATH_PASS', 'OWNED_VOLUME_AFTER_SUPERVISOR_DEATH_PASS',
    ]) and 'Power down' in serial and 'INTEGRATED_COMMAND_' not in serial
else:
    passed = all(marker in serial for marker in ['SANDBOX_NAMESPACE_FAILURE_ROLLBACK_PASS', 'SANDBOX_TWO_STEP_PIPELINE_PASS', 'SANDBOX_MOUNT_FAILURE_ROLLBACK_PASS', 'DOMAIN_PUBLICATION_MISMATCH_ROLLBACK_PASS', 'DOMAIN_PUBLICATION_ABSENT_ROLLBACK_PASS', 'DOMAIN_PUBLICATION_PRESENT_ROLLBACK_PASS', 'DOMAIN_PUBLICATION_MALFORMED_ROLLBACK_PASS', 'DOMAIN_CREATION_FAILURE_ROLLBACK_PASS', 'POLICY_INSTALLATION_FAILURE_ROLLBACK_PASS', 'INTEGRATED_EXECUTION_EXECUTE-SUCCESS_PASS', 'INTEGRATED_EXECUTION_EXECUTE-SHELL-FAILURE_PASS', 'INTEGRATED_EXECUTION_EXECUTE-TIMEOUT_PASS', 'INTEGRATED_EXECUTION_EXECUTE-SETUP_PASS', 'PREPARATION_REGISTRATION-FAILURE_ROLLBACK_PASS', 'PREPARATION_LAUNCH-FAILURE_ROLLBACK_PASS', 'PREPARATION_EXCHANGE-FAILURE_ROLLBACK_PASS', 'BOUND_POLICY_DESCRIPTOR_EXHAUSTION_RETRY_PASS', 'COMMAND_EXCEEDS_OLD_HELPER_LIMIT_PASS', 'PRIMARY_OUTCOME_THROUGH_FINALIZATION_RETRY_PASS', 'PRIMARY_OUTCOME_AFTER_CLEANUP_PASS', 'NONZERO_OUTCOME_THROUGH_CLEANUP_RETRY_PASS', 'FOREIGN_CONTROL_MOUNT_DESCRIPTORS_REJECTED_PASS', 'PRODUCTION_STEP_CANCEL_BEFORE_ADMISSION_PASS', 'PRODUCTION_STEP_OWNS_POLICY_AND_COMMAND_PASS', 'POLICY_PRIVATE_NAMESPACE_REJECTED_PASS', 'ADMISSION_TIMEOUT_CANCELS_WITHOUT_COMMAND_PASS', 'BOUND_POLICY_MISMATCH_RETAINS_OWNERS_PASS', 'BOUND_POLICY_CLEANUP_RETRY_RETAINS_IDENTITIES_PASS', 'NATIVE_FILESYSTEM_PIN_RETAINS_UNMOUNTED_SOURCE_PASS', 'NATIVE_REAPER_TIMEOUT_AND_RELEASE_PASS', 'SCOPED_ROOT_TERMINATION_PASS', 'SCOPED_ROOT_LAUNCH_FAILURE_AND_RETRY_PASS', 'NATIVE_MAPPED_NAMESPACE_CREATOR_REAPED_PASS', 'NATIVE_NAMESPACE_SETUP_REJECTIONS_PASS', 'NATIVE_NAMESPACE_IDENTITIES_AND_CREDENTIALS_PASS', 'INTEGRATED_COMMAND_SHELL-FAILURE_PASS', 'NATIVE_NAMESPACE_INIT_EXIT_STATUS_PASS', 'NATIVE_NAMESPACE_INIT_ADOPTION_PASS', 'INTEGRATED_LABEL_REMOUNT_PERSISTENCE_PASS', 'INTEGRATED_COMMAND_POLICY-OFF_PASS', 'NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS', 'NATIVE_PRIVILEGES_MISSING_CAP_REJECTED', 'NATIVE_PRIVILEGES_EXEC_PASS', 'INTEGRATED_COMMAND_SUCCESS_PASS', 'INTEGRATED_COMMAND_TIMEOUT_PASS', 'INTEGRATED_REMOUNT_PERSISTENCE_PASS', 'POLICY_FIXTURE_CLEANUP_PASS', 'IDMAP_PROBE_EXIT 0'])
(out / 'verdict.json').write_text(json.dumps({'passed': passed, 'full_sandbox_qualified': False}, indent=2))
assert passed, serial[-5000:]
print('PASS', out)
