#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Package locally; run privileged lifecycle assertions only in a disposable VM."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / 'validation/rewrite-command-integration'))
from runtime import GuestRuntime


class LifecycleRuntime(GuestRuntime):
    TOOLS = GuestRuntime.TOOLS + ('dmsetup',)
    MODULES = ('loop', 'ext4', 'dm-zero', 'dm-snapshot', 'virtio_pci', 'virtio_blk',
               'veth', 'nf_tables', 'nft_chain_nat', 'nft_masq', 'nft_ct')


class LifecycleRun:
    def __init__(self, args):
        self.args = args
        self.output = args.output.resolve()
        self.root = self.output / 'root'

    @staticmethod
    def digest(path):
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').hexdigest()

    @staticmethod
    def dependencies(executable):
        files = set()
        for line in subprocess.check_output(['ldd', str(executable)], text=True).splitlines():
            if 'not found' in line:
                raise RuntimeError(line)
            match = re.search(r'(/\S+)', line)
            if match:
                files.add(match.group(1).lstrip('/'))
        return files

    def run(self):
        self.output.mkdir()
        with tarfile.open(self.output / 'source.tar.gz', 'w:gz') as archive:
            for name in ('src', 'Cargo.toml', 'Cargo.lock', 'tests/lifecycle/src',
                         'tests/lifecycle/Cargo.toml', 'tests/lifecycle/Cargo.lock',
                         'tests/lifecycle/run.py', 'validation/rewrite-command-integration/runtime.py'):
                archive.add(REPO / name, arcname=name)
        env = os.environ.copy()
        env.update(CARGO_BUILD_JOBS='2', TMPDIR='/var/tmp/conglobate-rewrite-tmp')
        target = REPO / 'target/m7-stable'
        env['CARGO_TARGET_DIR'] = str(target)
        with (self.output / 'build.log').open('wb') as log:
            subprocess.run(['cargo', '+stable', 'build', '--locked', '--offline',
                            '--manifest-path', str(REPO / 'tests/lifecycle/Cargo.toml')],
                           env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        runtime = LifecycleRuntime(self.root, self.args.module_root.resolve(), self.args.kernel_release)
        runtime.create()
        binary = target / 'debug/conglobate-lifecycle-tests'
        runtime.executable(binary, Path('lifecycle-tests'))
        runtime.executable(binary, Path('usr/bin/formatter-stall'))
        runtime.executable(Path(shutil.which('sync')), Path('usr/bin/sync'))
        (self.root / 'conglobate-disposable-validation-vm').write_text('owned fixture VM\n')
        formatter = {'usr/bin/mke2fs'} | self.dependencies(shutil.which('mke2fs'))
        stalled = {'usr/bin/formatter-stall', 'conglobate-disposable-validation-vm'} | self.dependencies(binary)
        for name, files in [('formatter-runtime-files', formatter), ('formatter-stall-files', stalled)]:
            (self.root / name).write_text('\n'.join(sorted(files)) + '\n')
        mode = '' if self.args.drop is None else f'FORMATTER_DROP={self.args.drop} '
        setup = ('modprobe loop\nmodprobe dm-zero\nmodprobe dm-snapshot\nmodprobe ext4\n'
                 if self.args.case == 'formatter-failures' or self.args.case.startswith('sandbox-') else
                 'mkdir /work/cgroups\nmount -t cgroup2 cgroup2 /work/cgroups\n')
        if self.args.case == 'network-composite':
            setup = ''.join(f'modprobe {name}\n' for name in
                            ('veth', 'nf_tables', 'nft_chain_nat', 'nft_masq', 'nft_ct'))
        (self.root / 'fixture.sh').write_text(
            '#!/bin/bash\nset -eux\n' + setup + mode + '/lifecycle-tests ' + self.args.case + '\n')
        (self.root / 'init').write_text('''#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
mkdir -p /work /run
chmod 755 /run
mount -t tmpfs tmpfs /work
modprobe virtio_pci
modprobe virtio_blk
set +e
bash /fixture.sh
status=$?
echo "LIFECYCLE_EXIT $status"
printf 'LIFECYCLE_EXIT %s\\n' "$status" > /dev/vda
sync /dev/vda
echo o > /proc/sysrq-trigger
exec sleep infinity
''')
        (self.root / 'init').chmod(0o755)
        paths = sorted(self.root.rglob('*'))
        (self.output / 'inputs.json').write_text(json.dumps({str(p.relative_to(self.root)): self.digest(p)
            for p in paths if p.is_file() and not p.is_symlink()}, indent=2) + '\n')
        packed = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'],
            input=b'\0'.join(str(p.relative_to(self.root)).encode() for p in paths) + b'\0',
            cwd=self.root, capture_output=True, check=True).stdout
        (self.output / 'initramfs.gz').write_bytes(gzip.compress(packed, compresslevel=1, mtime=0))
        shutil.copy2(self.args.kernel, self.output / 'vmlinuz')
        status = self.output / 'status.raw'
        with status.open('wb') as stream:
            stream.truncate(4096)
        command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '4096',
            '-smp', '2', '-nodefaults', '-no-reboot', '-display', 'none', '-serial', 'stdio',
            '-monitor', 'none', '-kernel', str(self.output / 'vmlinuz'),
            '-initrd', str(self.output / 'initramfs.gz'),
            '-append', 'console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet',
            '-drive', f'file={status},format=raw,if=virtio']
        (self.output / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
        (self.output / 'runtime.json').write_text(json.dumps({'kernel_sha256':self.digest(self.output/'vmlinuz'),
            'kernel_release':self.args.kernel_release, 'drop':self.args.drop, 'case':self.args.case,
            'architecture':'native amd64 ordinary VM', 'network_attached':False}, indent=2) + '\n')
        with (self.output / 'serial.txt').open('wb') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=180, check=True)
        serial = (self.output / 'serial.txt').read_text(errors='replace')
        # Immediate abandonment need not schedule the child before VM disposal.
        markers = [] if self.args.drop == 'running' else ['FORMATTER_STALL_ENTERED']
        if self.args.case == 'domain-publication':
            markers = [f'DOMAIN_PUBLICATION_{mode}_ROLLBACK_PASS'
                       for mode in ('ABSENT', 'PRESENT', 'MALFORMED', 'MISMATCH')]
        elif self.args.case == 'contained':
            markers = ['CONTAINED_LAUNCH_FAILURE_CLEANUP_PASS', 'CONTAINED_JOIN_FAILURE_CLEANUP_PASS',
                       'CONTAINED_PRE_ADMISSION_DESCENDANT_DRAIN_PASS',
                       'CONTAINED_ABRUPT_HELPER_DEATH_DRAIN_PASS']
        elif self.args.case == 'network-composite':
            markers = ['NETWORK_NAMESPACE_FLOCK_EXCLUSION_AND_RELEASE_PASS',
                       'NETWORK_PRODUCTION_ALLOCATION_CONTENTION_BEFORE_MUTATION_PASS',
                       'NETWORK_COMPOSITE_AFTER_NAMESPACE_FAILURE_CLEANUP_PASS',
                       'NETWORK_COMPOSITE_NAT_FAILURE_PRESERVES_FOREIGN_TABLE_PASS',
                       'NETWORK_PRODUCTION_ALLOCATION_RELEASE_AND_DISTINCT_LIVE_PREFIX_PASS',
                       'NETWORK_COMPOSITE_EXPIRED_TEARDOWN_RETRY_RETAINS_OWNER_PASS']
        elif self.args.case == 'sandbox-supervisor-death':
            markers = ['FILESYSTEM_WRITER_UNINTERRUPTIBLE_PASS', 'SUPERVISOR_ABRUPT_DEATH_INJECTED']
        elif self.args.case == 'sandbox-blocked-io':
            markers = ['FILESYSTEM_WRITER_UNINTERRUPTIBLE_PASS',
                       'BLOCKED_IO_FIXED_DEADLINE_RETAINS_STORAGE_PASS']
        elif self.args.case == 'sandbox-gate-failure':
            markers = ['RECEIVER_CONTROL_HELD_DURING_GATE_FAILURE_CLEANUP_PASS',
                       'SANDBOX_GATE_FAILURE_ROLLBACK_PASS']
        elif self.args.case.startswith('sandbox-'):
            markers = [self.args.case.upper().replace('-', '_') + '_ROLLBACK_PASS']
        elif self.args.drop:
            markers += [f'FORMATTER_DROP_{self.args.drop.upper()}_RETAINS_STORAGE_PASS']
        else:
            markers += ['FORMATTER_DEADLINE_CANCEL_REAP_PASS', 'FORMATTER_EXHAUSTED_SNAPSHOT_REJECTED_PASS',
                        'FORMATTER_FAILURE_RESOURCES_RELEASED_PASS']
        checks = {marker: f'\n{marker}\n' in serial for marker in markers}
        if self.args.case == 'sandbox-supervisor-death':
            checks['supervisor_abort_status'] = status.read_bytes().rstrip(b'\0') == b'LIFECYCLE_EXIT 134\n'
        else:
            checks['exit_zero'] = status.read_bytes().rstrip(b'\0') == b'LIFECYCLE_EXIT 0\n'
        checks['vm_powered_off'] = 'Power down' in serial
        report = {'checks':checks, 'passed':all(checks.values()), 'full_lifecycle_qualified':False}
        (self.output / 'assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
        if not report['passed']:
            raise RuntimeError('lifecycle fixture failed; evidence retained')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--kernel', type=Path, required=True)
    parser.add_argument('--kernel-release', required=True)
    parser.add_argument('--module-root', type=Path, required=True)
    parser.add_argument('--case', choices=['formatter-failures', 'domain-publication', 'contained', 'sandbox-namespace-failure', 'sandbox-mount-failure', 'sandbox-launch-failure', 'sandbox-join-failure', 'sandbox-attached-exit', 'network-composite', 'sandbox-gate-failure', 'sandbox-blocked-io', 'sandbox-supervisor-death'], default='formatter-failures')
    parser.add_argument('--drop', choices=['running', 'wait'])
    args = parser.parse_args()
    if platform.machine() != 'x86_64':
        parser.error('native amd64/KVM required')
    if '/' in args.kernel_release or args.kernel_release in ('', '.', '..'):
        parser.error('invalid kernel release')
    if args.drop and args.case != 'formatter-failures':
        parser.error('--drop applies only to formatter-failures')
    LifecycleRun(args).run()
