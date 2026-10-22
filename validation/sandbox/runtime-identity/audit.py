#!/usr/bin/env python3
"""Read-only static audit of an owned guest fixture; never execute its binaries.

Not a loader emulator or proof of runtime closure. Output is created exclusively.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fixture', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    fixture = args.fixture.resolve(strict=True)
    inventory_path = fixture / 'inputs.json'
    inventory = json.loads(inventory_path.read_text())
    files = inventory['files']
    errors, elf, scripts = [], {}, {}

    def owned(name):
        path = fixture / name
        if not path.resolve(strict=True).is_relative_to(fixture):
            raise ValueError(f'fixture path escapes: {name}')
        return path

    for name, expected in sorted(files.items()):
        path = owned(name)
        if digest(path) != expected:
            errors.append(f'changed bytes: {name}')
        with path.open('rb') as stream:
            prefix = stream.read(256)
        if prefix.startswith(b'#!'):
            scripts[name] = prefix.splitlines()[0].decode(errors='replace')
        if not prefix.startswith(b'\x7fELF'):
            continue
        result = subprocess.run(['/usr/bin/readelf', '-lW', '-dW', str(path)],
                                capture_output=True, text=True, timeout=10,
                                env={'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
        if result.returncode:
            errors.append(f'readelf failed: {name}: {result.stderr}')
            continue
        text = result.stdout
        elf[name] = {
            'sha256': expected,
            'interpreter': re.findall(r'Requesting program interpreter: ([^\]]+)', text),
            'needed': re.findall(r'\(NEEDED\).*\[([^\]]+)\]', text),
            'rpath': re.findall(r'\(RPATH\).*\[([^\]]*)\]', text),
            'runpath': re.findall(r'\(RUNPATH\).*\[([^\]]*)\]', text),
        }
    for name, target in sorted(inventory['symlinks'].items()):
        # Parent containment is checked without dereferencing the final link.
        path = fixture / name
        if not path.parent.resolve(strict=True).is_relative_to(fixture):
            raise ValueError(f'link parent escapes: {name}')
        if not path.is_symlink() or os.readlink(path) != target:
            errors.append(f'changed link: {name}')

    listed = set(files) | set(inventory['symlinks'])
    for path in sorted((fixture / 'root').rglob('*')):
        if path.is_symlink() or path.is_file():
            name = str(path.relative_to(fixture))
            if name not in listed:
                errors.append(f'unlisted path: {name}')

    candidates = {}
    for name in files:
        if name.startswith('root/'):
            candidates.setdefault(Path(name).name, []).append(name)
    for item in elf.values():
        item['needed_basename_candidates'] = {
            name: candidates.get(name, []) for name in item['needed']}
        item['interpreter_present'] = {
            name: owned('root/' + name.lstrip('/')).is_file()
            for name in item['interpreter']}
    report = {
        'scope': 'static packaged identity, not dynamic resolution or immutability proof',
        'case': inventory['case'],
        'inventory_sha256': digest(inventory_path),
        'audit_sha256': digest(Path(__file__)),
        'readelf_sha256': digest(Path('/usr/bin/readelf')),
        'checked_files': len(files), 'checked_symlinks': len(inventory['symlinks']),
        'errors': errors, 'elf': elf, 'scripts': scripts,
        'packaged_etc': {name: sha for name, sha in files.items() if name.startswith('root/etc/')},
        'absent_loader_configs': [name for name in ('etc/ld.so.preload', 'etc/ld.so.cache', 'etc/ld.so.conf')
                                 if not (fixture / 'root' / name).exists()],
        'limits': ['Basename candidates do not model loader search order, ABI or hwcaps.',
                   'DT_NEEDED excludes dlopen, NSS, plugins, subprocesses and interpreted imports.',
                   'Inventory does not record mode, ownership, xattrs or directory permissions.',
                   'Matching retained files does not prove immutable runtime paths or attestation.'],
    }
    with args.out.open('x') as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write('\n')
    print(json.dumps({'files': len(files), 'elf': len(elf), 'scripts': len(scripts), 'errors': errors}))
    if errors:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
