#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Prepare an initial tool filesystem outside conglobate, in a disposable VM."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
from run import DistroRuntime, REPOSITORY


class SeedRuntime(DistroRuntime):
    TOOLS = DistroRuntime.TOOLS + ('gzip', 'sfdisk')


class SeedRun:
    def __init__(self, arguments):
        self.arguments = arguments
        self.output = arguments.output.resolve()
        self.root = self.output / 'root'

    def run(self):
        if not 0 < self.arguments.size_mib <= (2**63 - 1) // 1024**2 - 2:
            raise ValueError('--size-mib must be positive and fit a signed 64-bit file size')
        size = self.arguments.size_mib * 1024**2
        if platform.machine() != 'x86_64':
            raise RuntimeError('native amd64/KVM seed preparation only')
        source = self.arguments.archive.resolve()
        with source.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if digest != self.arguments.sha256:
            raise RuntimeError('seed archive identity mismatch')
        with tarfile.open(source) as archive:
            members = archive.getmembers()
            for member in members:
                path = Path(member.name)
                if path.is_absolute() or '..' in path.parts or path.name.startswith('.wh.'):
                    raise RuntimeError('requires a single complete rootfs archive without whiteouts')
        self.output.mkdir()
        with tarfile.open(self.output / 'source.tar.gz', 'w:gz') as archive:
            for name in ['tests/distro', 'tests/vm_runtime.py']:
                archive.add(REPOSITORY / name, arcname=name)
        runtime = SeedRuntime(self.root, self.arguments.module_root.resolve(), self.arguments.kernel_release)
        runtime.create()
        shutil.copy2(source, self.root / 'rootfs.tar.gz')
        (self.root / 'seed.sh').write_text('''#!/bin/bash
set -eux
for module in virtio_pci virtio_blk ext4; do modprobe "$module"; done
mkdir /target
printf 'label: gpt\\nstart=256, size=ROOT_SECTORS, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709\\n' | sfdisk /dev/vda
mke2fs -q -F -t ext4 -b 4096 -I 256 -i 65536 -m 0 -E root_owner=0:0,lazy_itable_init=0,lazy_journal_init=0 /dev/vda1
mount /dev/vda1 /target
tar --numeric-owner --same-owner --xattrs --xattrs-include='*' --acls -xzf /rootfs.tar.gz -C /target
stat -c 'SEED_OWNER %u %g %a %n' /target/etc/passwd /target/etc/shadow
sync
umount /target
e2fsck -fn /dev/vda1
echo SEED_FORMAT_EXTRACT_UNMOUNT_PASS
'''.replace('ROOT_SECTORS', str(size // 4096)))
        (self.root / 'init').write_text('''#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
set +e
bash /seed.sh
status=$?
echo "SEED_EXIT $status"
printf 'SEED_EXIT %s\\n' "$status" > /dev/vdb
sync
echo b > /proc/sysrq-trigger
exec sleep infinity
''')
        (self.root / 'init').chmod(0o755)
        paths = sorted(self.root.rglob('*'))
        packed = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'],
            input=b'\0'.join(str(p.relative_to(self.root)).encode() for p in paths) + b'\0',
            cwd=self.root, capture_output=True, check=True).stdout
        (self.output / 'initramfs.gz').write_bytes(gzip.compress(packed, compresslevel=1, mtime=0))
        shutil.copy2(self.arguments.kernel, self.output / 'vmlinuz')
        image = self.output / 'root.gpt'
        with image.open('wb') as stream:
            stream.truncate(size + 2 * 1024**2)
        result = self.output / 'status.raw'
        result.write_bytes(bytes(4096))
        command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '2048',
            '-smp', '2', '-nodefaults', '-no-reboot', '-display', 'none', '-serial', 'stdio',
            '-monitor', 'none', '-kernel', str(self.output / 'vmlinuz'),
            '-initrd', str(self.output / 'initramfs.gz'),
            '-append', 'console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet',
            '-drive', f'file={image},format=raw,if=none,id=image',
            '-device', 'virtio-blk-pci,drive=image,logical_block_size=4096,physical_block_size=4096',
            '-drive', f'file={result},format=raw,if=virtio']
        (self.output / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
        with (self.output / 'serial.txt').open('wb') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=240, check=True)
        serial = (self.output / 'serial.txt').read_text(errors='replace')
        exit_record = result.read_bytes().split(b'\0', 1)[0]
        passed = ('SEED_FORMAT_EXTRACT_UNMOUNT_PASS' in serial
                  and exit_record == b'SEED_EXIT 0\n')
        for name in ['etc/passwd', 'etc/shadow']:
            member = next(m for m in members if str(Path(m.name)) == name)
            expected = f'SEED_OWNER {member.uid} {member.gid} {member.mode:o} /target/{name}'
            passed = passed and expected in serial
        (self.output / 'verdict.json').write_text(json.dumps({
            'seed_filesystem_passed': passed, 'archive_sha256': digest,
            'archive_members': len(members), 'filesystem_bytes': size,
            'conglobate_build_passed': False,
        }, indent=2) + '\n')
        if not passed:
            raise RuntimeError('seed preparation failed; inspect serial.txt')
        print('SEED FILESYSTEM PASS; NOT A CONGLOBATE BUILD', image)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--size-mib', type=int, required=True,
                        help='Filesystem capacity in MiB, including space for later package installs')
    parser.add_argument('--kernel', type=Path, required=True)
    parser.add_argument('--kernel-release', required=True)
    parser.add_argument('--module-root', type=Path, required=True)
    SeedRun(parser.parse_args()).run()
