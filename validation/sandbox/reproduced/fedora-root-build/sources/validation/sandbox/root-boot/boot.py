#!/usr/bin/env python3
"""Boot only the owned, bounded Fedora fixture exports; never mount them on host."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--build', required=True, type=Path)
parser.add_argument('--out', required=True, type=Path)
parser.add_argument('--kernel-version', required=True)
args = parser.parse_args()
out = args.out.resolve()
out.mkdir()
firmware = args.build / 'boot-artifacts.bin'
assert firmware.stat().st_size <= 512 * 1024 * 1024 + 1024
contents = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'initramfs/boot.py'))['decode'](firmware.read_bytes())
for name, content in contents.items():
    (out / name).write_bytes(content)
export = args.build / 'root-artifact.bin'
assert export.stat().st_size <= 1024 * 1024 * 1024 + 256
with export.open('rb') as source, (out / 'rootfs.gz').open('xb') as dest:
    assert source.readline(128) == b'CONGLOBATE_ROOT_FIXTURE_V1\n'
    match = re.fullmatch(rb'([1-9][0-9]*) ([0-9a-f]{64})\n', source.readline(128))
    assert match
    size = int(match[1])
    assert size <= 1024 * 1024 * 1024
    digest = hashlib.sha256()
    remaining = size
    while remaining:
        chunk = source.read(min(1024 * 1024, remaining))
        assert chunk, 'truncated export'
        remaining -= len(chunk)
        digest.update(chunk)
        dest.write(chunk)
    assert not source.read(1)
    assert digest.hexdigest().encode() == match[2]
raw_digest = hashlib.sha256()
size = 0
with gzip.open(out / 'rootfs.gz', 'rb') as source, (out / 'rootfs.raw').open('xb') as dest:
    while chunk := source.read(1024 * 1024):
        size += len(chunk)
        assert size <= 2 * 1024**3
        raw_digest.update(chunk)
        if not any(chunk):
            dest.seek(len(chunk), 1)
        else:
            dest.write(chunk)
    assert size == 2 * 1024**3
    dest.truncate(size)
identity = {'firmware': {name: hashlib.sha256(data).hexdigest() for name, data in contents.items()},
            'rootfs_compressed_sha256': digest.hexdigest(),
            'rootfs_raw_sha256': raw_digest.hexdigest(), 'rootfs_raw_bytes': size}
(out / 'artifacts.json').write_text(json.dumps(identity, indent=2) + '\n')
command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '2048', '-smp', '2',
           '-nodefaults', '-no-reboot', '-display', 'none', '-serial', 'stdio', '-monitor', 'none',
           '-kernel', str(out / 'kernel'), '-initrd', str(out / 'initramfs'),
           '-drive', f'file={out / "rootfs.raw"},format=raw,if=none,id=owned,snapshot=on',
           '-device', 'virtio-blk-pci,drive=owned', '-append',
           'console=ttyS0 root=/dev/vda rw selinux=0 panic=-1 rd.shell=0 systemd.unit=multi-user.target']
(out / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
with (out / 'results.txt').open('wb') as log:
    subprocess.run(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                   timeout=150, check=True, env=os.environ | {'TMPDIR': str(out)})
text = (out / 'results.txt').read_text()
assert text.splitlines().count('FEDORA_ROOT_SWITCH_SYSTEMD_PASS ' + args.kernel_version) == 1
assert 'ROOT_BOOT_PROBE_FAILED' not in text
with (out / 'rootfs.raw').open('rb') as source:
    assert hashlib.file_digest(source, 'sha256').hexdigest() == raw_digest.hexdigest()
print('FEDORA_EXPORTED_ROOT_BOOT_AND_UNCHANGED_ARTIFACT_PASS')
