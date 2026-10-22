#!/usr/bin/env python3
"""Boot only the two bounded, hash-verified artifacts exported by the fixture."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess

LIMIT = 256 * 1024 * 1024


def decode(data):
    if len(data) > 2 * LIMIT + 1024:
        raise ValueError('oversize export')
    stream = io.BytesIO(data)
    if stream.readline(128) != b'CONGLOBATE_BOOT_FIXTURE_V1\n':
        raise ValueError('wrong export magic')
    artifacts = {}
    for name in ('kernel', 'initramfs'):
        header = stream.readline(128)
        match = re.fullmatch(rb'([1-9][0-9]*) ([0-9a-f]{64})\n', header)
        if match is None:
            raise ValueError('invalid artifact header')
        size = int(match[1])
        if size > LIMIT:
            raise ValueError('oversize artifact')
        content = stream.read(size)
        if len(content) != size or hashlib.sha256(content).hexdigest().encode() != match[2]:
            raise ValueError('truncated or corrupt artifact')
        artifacts[name] = content
    if stream.read(1):
        raise ValueError('trailing export data')
    return artifacts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--export', type=Path, required=True)
    parser.add_argument('--kernel-version', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.export.stat().st_size > 2 * LIMIT + 1024:
        raise ValueError('oversize export file')
    contents = decode(args.export.read_bytes())
    out = args.out.resolve()
    out.mkdir()
    for name, content in contents.items():
        (out / name).write_bytes(content)
    inventory = {name: {'size': len(content), 'sha256': hashlib.sha256(content).hexdigest()}
                 for name, content in contents.items()}
    (out / 'artifacts.json').write_text(json.dumps(inventory, indent=2) + '\n')
    command = ['qemu-system-x86_64', '-enable-kvm', '-cpu', 'host', '-m', '2048',
               '-smp', '2', '-nodefaults', '-no-reboot', '-display', 'none',
               '-serial', 'stdio', '-monitor', 'none', '-kernel', str(out / 'kernel'),
               '-initrd', str(out / 'initramfs'), '-append',
               'console=ttyS0 selinux=0 panic=-1 rd.shell=0']
    (out / 'command.json').write_text(json.dumps(command, indent=2) + '\n')
    with (out / 'results.txt').open('wb') as stream:
        subprocess.run(command, stdin=subprocess.DEVNULL, stdout=stream,
                       stderr=subprocess.STDOUT, timeout=90, check=True)
    result = (out / 'results.txt').read_text()
    wanted = 'INITRAMFS_SYSTEMD_MODULE_BOOT_PASS ' + args.kernel_version
    assert result.splitlines().count(wanted) == 1, 'missing or duplicate boot milestone'
    print('Booted exported kernel/initramfs through systemd, udev and fuse module loading')


if __name__ == '__main__':
    main()
