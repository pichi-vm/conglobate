#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Build a real distro in a disposable VM; no privileged host operations."""
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

REPOSITORY = Path(__file__).resolve().parents[2]
# This shared runtime constructor must migrate to maintained tests before M8.
sys.path.insert(0, str(REPOSITORY / 'validation/rewrite-command-integration'))
from runtime import GuestRuntime


class DistroRuntime(GuestRuntime):
    TOOLS = GuestRuntime.TOOLS + ('ip', 'tar', 'sync')
    MODULES = GuestRuntime.MODULES + ('virtio_pci', 'virtio_blk', 'virtio_net')


class DistroRun:
    def __init__(self, arguments):
        self.arguments = arguments
        self.output = arguments.output.resolve()
        self.root = self.output / 'root'

    def run(self):
        if platform.machine() != 'x86_64':
            raise RuntimeError('this runner currently qualifies only native amd64/KVM')
        if bool(self.arguments.firmware_pmi) != bool(self.arguments.firmware_dtb):
            raise RuntimeError('firmware PMI and DTB must be supplied together')
        self.output.mkdir()
        with tarfile.open(self.output / 'source.tar.gz', 'w:gz') as snapshot:
            for name in ('src', 'policy', 'Cargo.toml', 'Cargo.lock', 'build.rs',
                         'tests/distro', 'validation/rewrite-command-integration/runtime.py'):
                snapshot.add(REPOSITORY / name, arcname=name)
        env = os.environ.copy()
        env.update(CARGO_BUILD_JOBS='2', CARGO_TARGET_DIR=str(REPOSITORY / 'target/m7-stable'),
                   TMPDIR='/var/tmp/conglobate-rewrite-tmp')
        with (self.output / 'build.log').open('wb') as log:
            subprocess.run(['cargo', '+stable', 'build', '--locked'], cwd=REPOSITORY,
                           env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        runtime = DistroRuntime(self.root, self.arguments.module_root.resolve(), self.arguments.kernel_release)
        runtime.create()
        runtime.executable(REPOSITORY / 'target/m7-stable/debug/conglobate', Path('usr/bin/conglobate'))
        # Enumerate only the trusted host formatter, never an image executable.
        formatter = Path(shutil.which('mke2fs'))
        formatter_files = {'usr/bin/mke2fs'}
        for line in subprocess.check_output(['ldd', str(formatter)], text=True).splitlines():
            if 'not found' in line:
                raise RuntimeError('incomplete trusted formatter runtime')
            match = re.search(r'(/\S+)', line)
            if match:
                formatter_files.add(match.group(1).lstrip('/'))
        (self.root / 'formatter.json').write_text(json.dumps({
            'root': '/', 'executable': 'usr/bin/mke2fs', 'files': sorted(formatter_files),
        }, indent=2) + '\n')
        shutil.copytree(self.arguments.cache, self.root / 'cache')
        context = self.root / 'context'
        shutil.copytree(self.arguments.recipe.resolve().parent, context)
        selected = context / self.arguments.recipe.name
        if selected.name != 'recipe.yaml':
            shutil.copy2(selected, context / 'recipe.yaml')
        if self.arguments.firmware_pmi:
            firmware = context / 'firmware'
            firmware.mkdir()
            shutil.copy2(self.arguments.firmware_pmi, firmware / 'boot.pmi')
            shutil.copy2(self.arguments.firmware_dtb, firmware / 'base.dtb')
        (context / 'images.lock').write_text(
            f'version: 1\nimages:\n  {self.arguments.reference}:\n    index: {self.arguments.pin}\n')
        for command in [['init', '-q'], ['add', '.'],
                        ['-c', 'user.name=Qualification', '-c', 'user.email=qualification@invalid',
                         'commit', '-qm', 'Pin qualification inputs']]:
            subprocess.run(['git', '-C', str(context), *command], check=True)
        commit = subprocess.check_output(['git', '-C', str(context), 'rev-parse', 'HEAD'], text=True).strip()
        subprocess.run(['git', '-C', str(context), 'bundle', 'create',
                        str(self.output / 'context.bundle'), '--all'], check=True)
        (self.root / 'etc/resolv.conf').write_text('nameserver 10.0.2.3\n')
        (self.root / 'conglobate-disposable-validation-vm').touch()
        (self.root / 'build.sh').write_text(f'''#!/bin/bash
set -eux
modprobe virtio_pci
modprobe virtio_net
modprobe virtio_blk
for module in loop ext4 dm-zero dm-snapshot dm-verity veth nf_tables nft_chain_nat nft_masq nft_ct; do modprobe "$module"; done
ip link set lo up
# Exactly one supplied virtio NIC; use its kernel name rather than naming policy.
interface=
for candidate in /sys/class/net/*; do
    test "${{candidate##*/}}" = lo && continue
    test -z "$interface"
    interface=${{candidate##*/}}
done
test -n "$interface"
ip link set "$interface" up
ip address add 10.0.2.15/24 dev "$interface"
ip route add default via 10.0.2.2
echo 1 > /proc/sys/net/ipv4/ip_forward
echo 1 > /proc/sys/net/ipv6/conf/all/forwarding
conglobate build --repository /context --commit {commit} --recipe recipe.yaml --lock images.lock --cache /cache --output /work/output --resolv-conf /etc/resolv.conf --formatter-runtime /formatter.json
# Only the trusted VM supervisor sees this export device. The build workspace
# remains RAM-backed; commands receive neither the device nor an export path.
tar -C /work/output -cf /dev/vda .
sync
echo DISTRO_BUILD_AND_EXPORT_PASS
''')
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
set +e
bash /build.sh
status=$?
echo "DISTRO_BUILD_EXIT $status"
echo b > /proc/sysrq-trigger
exec sleep infinity
''')
        (self.root / 'init').chmod(0o755)
        paths = sorted(self.root.rglob('*'))
        (self.output / 'inputs.json').write_text(json.dumps({
            str(p.relative_to(self.root)): self.digest(p) for p in paths if p.is_file() and not p.is_symlink()
        }, indent=2) + '\n')
        archive = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'],
            input=b'\0'.join(str(p.relative_to(self.root)).encode() for p in paths) + b'\0',
            cwd=self.root, capture_output=True, check=True).stdout
        (self.output / 'initramfs.gz').write_bytes(gzip.compress(archive, compresslevel=1, mtime=0))
        shutil.copy2(self.arguments.kernel, self.output / 'vmlinuz')
        export = self.output / 'export.raw'
        with export.open('wb') as stream:
            stream.truncate(2 * 1024**3)
        command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '8192',
            '-smp', '2', '-nodefaults', '-no-reboot', '-display', 'none', '-serial', 'stdio',
            '-monitor', 'none', '-kernel', str(self.output / 'vmlinuz'),
            '-initrd', str(self.output / 'initramfs.gz'),
            '-append', 'console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet',
            '-netdev', 'user,id=outbound', '-device', 'virtio-net-pci,netdev=outbound',
            '-drive', f'file={export},format=raw,if=virtio']
        (self.output / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
        with (self.output / 'serial.txt').open('wb') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=2400, check=True)
        serial = (self.output / 'serial.txt').read_text(errors='replace')
        passed = 'DISTRO_BUILD_AND_EXPORT_PASS' in serial and 'DISTRO_BUILD_EXIT 0' in serial
        (self.output / 'verdict.json').write_text(json.dumps({
            'build_passed': passed, 'boot_passed': False, 'full_distro_qualified': False,
        }, indent=2) + '\n')
        if not passed:
            raise RuntimeError('distro build failed; inspect preserved serial.txt')
        with tarfile.open(export) as archive:
            archive.extractall(self.output / 'artifact', filter='data')
        for blob in (self.output / 'artifact/blobs/sha256').iterdir():
            if not blob.is_file() or self.digest(blob) != blob.name:
                raise RuntimeError(f'exported artifact digest mismatch: {blob.name}')
        print('BUILD PASS; BOOT NOT YET TESTED', self.output)

    @staticmethod
    def digest(path):
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').hexdigest()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--kernel', type=Path, required=True)
    parser.add_argument('--kernel-release', required=True)
    parser.add_argument('--module-root', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--pin', required=True)
    parser.add_argument('--recipe', type=Path, required=True)
    parser.add_argument('--reference', default='qualification.local/fedora:43')
    parser.add_argument('--firmware-pmi', type=Path)
    parser.add_argument('--firmware-dtb', type=Path)
    DistroRun(parser.parse_args()).run()
