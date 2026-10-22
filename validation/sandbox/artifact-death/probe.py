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

# Cancellation is cooperative: explicitly prove the handler has run at the
# stopped boundary before continuing the publication code.
for stage in ('copy', 'before-check', 'before-rename', 'after-rename'):
    root = Path('/work') / f'cancel-{stage}'
    root.mkdir(mode=0o700)
    Path('/work/reached').unlink(missing_ok=True)
    child = subprocess.Popen(['/artifact-death', str(root)],
        env=dict(os.environ, ARTIFACT_STOP=stage, ARTIFACT_CANCEL_RESUME='1'),
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    pidfd = os.pidfd_open(child.pid)
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert child.poll() is None
            if Path('/work/reached').exists() and '\nState:\tT' in Path(f'/proc/{child.pid}/status').read_text():
                break
            time.sleep(.01)
        else:
            raise AssertionError(f'cancellation did not reach stop: {stage}')
        assert Path('/work/reached').read_text() == stage
        expected = content(root / 'private')
        signal.pidfd_send_signal(pidfd, signal.SIGTERM)
        signal.pidfd_send_signal(pidfd, signal.SIGCONT)
        output, _ = child.communicate(timeout=5)
        print(output, end='', flush=True)
        assert f'ARTIFACT_GATE_CANCEL_FLAG_OBSERVED {stage}' in output
        late = stage in ('before-rename', 'after-rename')
        assert child.returncode == (0 if late else 1), (stage, child.returncode, output)
        if late:
            assert 'ARTIFACT_PUBLICATION_RETURNED' in output
            assert content(root / 'output') == expected
            print('ARTIFACT_LATE_CANCELLATION_PUBLISHES_LIMITATION', stage, flush=True)
        else:
            assert 'build cancelled' in output
            assert 'ARTIFACT_PUBLICATION_RETURNED' not in output
            assert not (root / 'output').exists()
            print('ARTIFACT_PRECHECK_CANCELLATION_REJECTED', stage, flush=True)
        assert not list(root.glob('.conglobate-output-*'))
        assert content(root / 'private') == expected
    finally:
        if child.poll() is None:
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            child.wait(timeout=5)
        os.close(pidfd)
print('ARTIFACT_CANCELLATION_BOUNDARY_MATRIX_PASS', flush=True)
