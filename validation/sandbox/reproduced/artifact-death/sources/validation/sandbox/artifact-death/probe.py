#!/usr/bin/python3
"""Bounded publication process-death visibility and leftover-state checks."""
import hashlib
import os
from pathlib import Path
import signal
import subprocess
import time

def content(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}

for stage in ('copy', 'before-rename', 'after-rename'):
    root = Path('/work') / stage
    root.mkdir(mode=0o700)
    Path('/work/reached').unlink(missing_ok=True)
    child = subprocess.Popen(['/artifact-death', str(root)],
                             env=dict(os.environ, ARTIFACT_STOP=stage))
    pidfd = os.pidfd_open(child.pid)
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert child.poll() is None
            if Path('/work/reached').exists() and '\nState:\tT' in Path(f'/proc/{child.pid}/status').read_text():
                break
            time.sleep(.01)
        else:
            raise AssertionError(f'publication did not reach stop: {stage}')
        assert Path('/work/reached').read_text() == stage
        expected = content(root / 'private')
        assert 'inventory.cbor' in expected and 'image.oci/index.json' in expected
        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
        assert child.wait(timeout=5) == -signal.SIGKILL
    finally:
        if child.poll() is None:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            child.wait(timeout=5)
        os.close(pidfd)
    staging = list(root.glob('.conglobate-output-*'))
    if stage == 'after-rename':
        assert not staging
        assert content(root / 'output') == expected
        print('ARTIFACT_DEATH_COMPLETE_OUTPUT_WITHOUT_RETURN', flush=True)
    else:
        assert not (root / 'output').exists()
        assert len(staging) == 1
        assert staging[0].stat().st_mode & 0o777 == 0o700
        actual = content(staging[0])
        if stage == 'before-rename':
            assert actual == expected
        else:
            assert actual and len(actual) < len(expected), (actual, expected)
        print('ARTIFACT_DEATH_PRIVATE_STAGING_REMAINS', stage, flush=True)
    assert content(root / 'private') == expected
    print('ARTIFACT_DEATH_VISIBILITY_PASS', stage, flush=True)
print('ARTIFACT_DEATH_MATRIX_PASS', flush=True)
