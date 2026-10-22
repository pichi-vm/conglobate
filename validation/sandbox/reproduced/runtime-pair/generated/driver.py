#!/usr/bin/python3
"""Owned dual-root path controls. No recipe or outside filesystem operations."""
from pathlib import Path
import shutil
import subprocess
cases = [('valid', None, None, None)]
cases += [(f'{root}-{name}-{kind}', root, name, kind)
          for root in ('tool', 'image') for name in ('proc', 'dev', 'sys')
          for kind in ('link', 'file', 'missing')]
for label, selected_root, selected_name, kind in cases:
    canary = Path('/work/canary'); canary.mkdir()
    (canary/'sentinel').write_text('owned canary unchanged')
    for root in ('tool', 'image'):
        path = Path('/work')/root; path.mkdir()
        for name in ('proc', 'dev', 'sys'):
            target = path/name
            if (root, name) == (selected_root, selected_name):
                if kind == 'link': target.symlink_to(canary)
                elif kind == 'file': target.write_text('regular fixture file')
            else: target.mkdir()
    Path('/work/tool/output').mkdir()
    before = Path('/proc/self/mountinfo').read_text()
    result = subprocess.run(['unshare', '--user', '--mount', '--pid', '--ipc', '--uts', '--net', '--fork',
        '--map-users', '0:100000:65536', '--map-groups', '0:100000:65536', '--setuid', '0', '--setgid', '0',
        '/bin/bash', '/command-launch'], capture_output=True, text=True, timeout=10)
    if label == 'valid':
        assert result.returncode == 0, result.stderr
        assert 'DUAL_RUNTIME_FLAGS_AND_TYPES_PASS' in result.stdout
        print(result.stdout, end='', flush=True)
    else:
        assert result.returncode != 0
        assert f'runtime target {selected_name}:' in result.stderr, result.stderr
        assert ('ENOENT' if kind == 'missing' else 'ENOTDIR') in result.stderr
        assert 'DUAL_RUNTIME_ALL_TARGETS_PINNED' not in result.stdout
        assert 'DUAL_RUNTIME_MOUNTS_INSTALLED' not in result.stdout
        print('DUAL_RUNTIME_REJECTED_BEFORE_MOUNT', label, flush=True)
    assert Path('/proc/self/mountinfo').read_text() == before
    assert (canary/'sentinel').read_text() == 'owned canary unchanged'
    for root in ('tool', 'image', 'canary'): shutil.rmtree(Path('/work')/root)
print('DUAL_RUNTIME_MATRIX_PASS', flush=True)
