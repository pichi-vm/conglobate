#!/usr/bin/python3
"""Owned-path negative controls for safe runtime target pinning."""
from pathlib import Path
import shutil
import subprocess
for mode in ('directories', 'proc-link', 'dev-link', '.old-link', 'proc-file', 'dev-file',
             'proc-missing', 'dev-missing'):
    root = Path('/work/view'); root.mkdir()
    canary = Path('/work/canary'); canary.mkdir()
    (canary / 'sentinel').write_text('owned fixture unchanged')
    for name in ('proc', 'dev', '.old'):
        if mode == name + '-link': (root / name).symlink_to(canary)
        elif mode == name + '-file': (root / name).write_text('not a directory')
        elif mode != name + '-missing': (root / name).mkdir()
    Path('/work/mode').write_text(mode)
    before = Path('/proc/self/mountinfo').read_text()
    result = subprocess.run(['unshare', '--user', '--mount', '--pid', '--ipc', '--uts', '--net', '--fork',
                    '--map-users', '0:100000:65536', '--map-groups', '0:100000:65536',
                    '--setuid', '0', '--setgid', '0', '/bin/bash', '/command-launch'],
                   capture_output=True, text=True, timeout=10)
    if mode == 'directories':
        assert result.returncode == 0, result.stderr
        assert 'PINNED_RUNTIME_ENTRY_VALID_CONTROL_PASS' in result.stdout
        print(result.stdout, end='', flush=True)
    else:
        assert result.returncode != 0
        name = mode.split('-')[0]
        assert 'runtime target ' + name + ':' in result.stderr, result.stderr
        assert ('ENOENT' if mode.endswith('missing') else 'ENOTDIR') in result.stderr
        assert 'RUNTIME_TARGETS_PINNED' not in result.stdout
        assert 'PINNED_RUNTIME_MOUNTS_INSTALLED' not in result.stdout
        print('PINNED_RUNTIME_TARGET_REJECTED_BEFORE_MOUNT', mode, flush=True)
    assert Path('/proc/self/mountinfo').read_text() == before
    assert (canary / 'sentinel').read_text() == 'owned fixture unchanged'
    shutil.rmtree(root); shutil.rmtree(canary)
print('PINNED_RUNTIME_TARGET_MATRIX_PASS', flush=True)
