#!/usr/bin/env python3
"""Bounded identity-audit controls using disposable text files only."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

audit = Path(__file__).with_name('audit.py')
with tempfile.TemporaryDirectory(prefix='conglobate-identity-controls-') as directory:
    fixture = Path(directory)
    root = fixture / 'root'
    root.mkdir()
    data = root / 'owned'
    data.write_bytes(b'fixture\n')
    link = root / 'link'
    link.symlink_to('owned')
    (fixture / 'inputs.json').write_text(json.dumps({
        'case': 'owned-text-control',
        'files': {'root/owned': hashlib.sha256(data.read_bytes()).hexdigest()},
        'symlinks': {'root/link': 'owned'},
    }))

    def check(name, expected):
        output = fixture / (name + '.json')
        result = subprocess.run(['python3', str(audit), str(fixture), '--out', str(output)],
                                capture_output=True, text=True, timeout=10)
        assert result.returncode == bool(expected), (name, result.stdout, result.stderr)
        assert json.loads(output.read_text())['errors'] == expected, name
        print('RUNTIME_IDENTITY_AUDIT_CONTROL_PASS', name)

    check('unchanged', [])
    data.write_bytes(b'changed fixture\n')
    check('changed-bytes', ['changed bytes: root/owned'])
    data.write_bytes(b'fixture\n')
    link.unlink()
    link.symlink_to('./owned')
    check('changed-link', ['changed link: root/link'])
    link.unlink()
    link.symlink_to('owned')
    (root / 'extra').write_text('owned extra fixture\n')
    check('unlisted-file', ['unlisted path: root/extra'])
