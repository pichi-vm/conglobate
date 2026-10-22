#!/usr/bin/env python3
"""Reject changed/missing fixture RPMs before any guest/output is created."""
import argparse
from pathlib import Path
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rpms', type=Path, required=True)
args = parser.parse_args()
with tempfile.TemporaryDirectory(prefix='conglobate-rpm-input-check-') as directory:
    root = Path(directory)
    packages = root / 'rpms'
    packages.mkdir()
    for source in sorted(args.rpms.glob('*.rpm')):
        (packages / source.name).symlink_to(source.resolve())
    first = next(iter(sorted(packages.glob('*.rpm'))))
    original = first.resolve()
    first.unlink()
    command = [sys.executable, str(Path(__file__).resolve().parents[1] / 'policy/package.py'),
               '--seed', str(root / 'unused-seed'), '--out', str(root / 'must-not-exist'),
               '--restricted-bootstrap', '--initramfs-rpms', str(packages)]
    for expected in ('RPM set differs', 'RPM digest mismatch'):
        if expected == 'RPM digest mismatch':
            first.write_bytes(original.read_bytes() + b'bounded fixture mutation\n')
        result = subprocess.run(command, capture_output=True, text=True, timeout=20)
        assert result.returncode != 0 and expected in result.stderr, result.stderr
        assert not (root / 'must-not-exist').exists()
        print('REJECTED_BEFORE_OUTPUT', expected)
