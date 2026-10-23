#!/usr/bin/env python3
"""Construct and run a disconnected disposable VM; never load host policy."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rewrite-command-integration'))
from runtime import GuestRuntime


class Validation:
    def __init__(self, args):
        self.args = args
        self.output = args.output.resolve()

    def run(self):
        self.output.mkdir()
        root = self.output / 'root'
        runtime = GuestRuntime(root, Path('/'), self.args.kernel_release)
        runtime.create()
        for name in ['labels.bpf.c', 'loader.c', 'probe.c', 'run.py']:
            shutil.copy2(HERE / name, self.output / name)
        includes = ['-I' + str(self.args.headers)]
        subprocess.run(['clang', '-O2', '-g', '-Wall', '-Werror', '-target', 'bpf',
                        *includes, '-c', str(HERE / 'labels.bpf.c'), '-o', str(root / 'labels.bpf.o')], check=True)
        subprocess.run(['cc', '-O2', '-Wall', '-Wextra', '-Werror', *includes,
                        str(HERE / 'loader.c'), '/usr/lib64/libbpf.so.1', '-o', str(self.output / 'loader')], check=True)
        subprocess.run(['cc', '-O2', '-Wall', '-Wextra', '-Werror', str(HERE / 'probe.c'),
                        '-o', str(self.output / 'probe')], check=True)
        runtime.executable(self.output / 'loader', Path('loader'))
        runtime.executable(self.output / 'probe', Path('probe'))
        (root / 'fixture').write_text('''#!/bin/bash
set -eux
touch /run/global-label-fixture
modprobe loop
modprobe ext4
truncate -s 32M /image
mke2fs -q -F -t ext4 /image
mkdir /assigned /readonly
mount -o loop /image /assigned
touch /assigned/file /assigned/read-only-mode
chown 65536:65536 /assigned/file /assigned/read-only-mode
chmod 0444 /assigned/read-only-mode
mount --bind /assigned /readonly
mount -o remount,bind,ro /readonly
probe() {
  setpriv --reuid=65536 --regid=65536 --clear-groups \
    unshare --user --map-root-user \
    setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all /probe "$1"
}
probe off
mount -t bpf bpf /sys/fs/bpf
/loader
probe on
rm /sys/fs/bpf/label-exception
probe off
umount /readonly
umount /assigned
echo GLOBAL_POLICY_FIXTURE_PASS
''')
        (root / 'init').write_text('''#!/bin/bash
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
bash /fixture
status=$?
echo "FIXTURE_EXIT $status"
echo b > /proc/sysrq-trigger
exec sleep infinity
''')
        (root / 'init').chmod(0o755)
        paths = sorted(root.rglob('*'))
        (self.output / 'inputs.json').write_text(json.dumps({str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in paths if p.is_file() and not p.is_symlink()}, indent=2) + '\n')
        packed = subprocess.run(['cpio', '--null', '-o', '--format=newc', '--owner=0:0'],
            input=b'\0'.join(str(p.relative_to(root)).encode() for p in paths) + b'\0',
            cwd=root, capture_output=True, check=True).stdout
        (self.output / 'initramfs.gz').write_bytes(gzip.compress(packed, compresslevel=1, mtime=0))
        shutil.copy2(self.args.kernel, self.output / 'vmlinuz')
        command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '2048',
            '-smp', '2', '-nodefaults', '-no-reboot', '-display', 'none', '-serial', 'stdio',
            '-monitor', 'none', '-kernel', str(self.output / 'vmlinuz'),
            '-initrd', str(self.output / 'initramfs.gz'),
            '-append', 'console=ttyS0 rdinit=/init selinux=0 lsm=capability,bpf panic=-1 quiet']
        (self.output / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
        with (self.output / 'serial.txt').open('wb') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=90, check=True)
        serial = (self.output / 'serial.txt').read_text(errors='replace')
        passed = all(marker in serial for marker in ['GLOBAL_EXACT_LABEL_BOUNDARY_PASS', 'GLOBAL_POLICY_FIXTURE_PASS', 'FIXTURE_EXIT 0'])
        (self.output / 'verdict.json').write_text(json.dumps({'mechanism_passed': passed,
            'production_lifecycle_validated': False}, indent=2) + '\n')
        if not passed:
            raise RuntimeError('fixture failed; inspect preserved serial.txt')
        print('GLOBAL POLICY MECHANISM PASS; NOT PRODUCTION QUALIFICATION')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--kernel', type=Path, required=True)
    parser.add_argument('--kernel-release', required=True)
    parser.add_argument('--headers', type=Path, required=True)
    Validation(parser.parse_args()).run()
