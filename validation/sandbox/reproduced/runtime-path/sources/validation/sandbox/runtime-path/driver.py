#!/usr/bin/python3
"""Exercise actual runtime mount setup only with owned guest canary paths."""
from pathlib import Path
import shutil
import subprocess
root = Path('/work/view')
for mode in ('directories', 'proc-link', 'dev-link', 'both-links'):
    root.mkdir()
    (root / '.old').mkdir()
    for name in ('proc', 'dev'):
        target = Path('/work/canary-' + name)
        target.mkdir()
        (target / 'sentinel').write_text('owned fixture unchanged')
        if mode == 'both-links' or mode == name + '-link':
            (root / name).symlink_to(target)
        else:
            (root / name).mkdir()
    Path('/work/mode').write_text(mode)
    before = Path('/proc/self/mountinfo').read_text()
    subprocess.run(['unshare', '--user', '--mount', '--pid', '--ipc', '--uts', '--net', '--fork',
                    '--map-users', '0:100000:65536', '--map-groups', '0:100000:65536',
                    '--setuid', '0', '--setgid', '0', '/bin/bash', '/command-launch'],
                   check=True, timeout=10)
    assert Path('/proc/self/mountinfo').read_text() == before
    for name in ('proc', 'dev'):
        target = Path('/work/canary-' + name)
        assert (target / 'sentinel').read_text() == 'owned fixture unchanged'
        shutil.rmtree(target)
    shutil.rmtree(root)
    print('RUNTIME_PATH_OUTER_CANARIES_UNCHANGED', mode, flush=True)
print('RUNTIME_PATH_MATRIX_COMPLETE', flush=True)
