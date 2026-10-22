#!/usr/bin/python3
"""Kill bounded fixture committers around rename, using completed build bytes."""
import hashlib
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time
assert int(Path('/work/owner-status').read_text()) == 0
source = Path('/work/published')
assert source.joinpath('inventory.cbor').is_file()
def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}
expected = hashes(source)
for stage in ('before-rename', 'after-rename', 'after-sync', 'collision'):
    root = Path('/work/commit-' + stage)
    root.mkdir()
    shutil.copytree(source, root / 'published')
    selected = 'before-rename' if stage == 'collision' else stage
    child = subprocess.Popen(['/commit-output', str(root)],
        env={**os.environ, 'COMMIT_STOP': selected}, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    fd = os.pidfd_open(child.pid)
    try:
        deadline = time.monotonic() + 5
        while not ((root / 'reached').exists() and
                   '\nState:\tT' in Path(f'/proc/{child.pid}/status').read_text()):
            assert child.poll() is None
            assert time.monotonic() < deadline
            time.sleep(.01)
        assert (root / 'reached').read_text() == selected
        if stage == 'collision':
            # Create a genuine destination after publisher has started.
            (root / 'result').mkdir()
            (root / 'result/sentinel').write_bytes(b'pre-existing fixture destination')
            identity = (root / 'result').stat().st_ino
            signal.pidfd_send_signal(fd, signal.SIGCONT)
            out, err = child.communicate(timeout=5)
            assert child.returncode == 1 and b'EEXIST' in err, (child.returncode, err)
            assert (root / 'result').stat().st_ino == identity
            assert (root / 'result/sentinel').read_bytes() == b'pre-existing fixture destination'
            assert hashes(root / 'published') == expected
            assert b'OUTER_COMMIT_RETURNED' not in out
            print('OUTER_COMMIT_CONCURRENT_DESTINATION_PRESERVED_PASS', flush=True)
        else:
            signal.pidfd_send_signal(fd, signal.SIGKILL)
            out, err = child.communicate(timeout=5)
            assert child.returncode == -signal.SIGKILL
            assert b'OUTER_COMMIT_RETURNED' not in out
            if stage == 'before-rename':
                assert not (root / 'result').exists()
                assert hashes(root / 'published') == expected
                retry = subprocess.run(['/commit-output', str(root)], capture_output=True)
                assert retry.returncode == 0 and b'OUTER_COMMIT_RETURNED' in retry.stdout
                assert hashes(root / 'result') == expected
            else:
                assert not (root / 'published').exists()
                assert hashes(root / 'result') == expected
                # A rebuilt/re-staged retry must not overwrite the committed output.
                identity = (root / 'result').stat().st_ino
                shutil.copytree(source, root / 'published')
                retry = subprocess.run(['/commit-output', str(root)], capture_output=True)
                assert retry.returncode == 1 and b'EEXIST' in retry.stderr
                assert (root / 'result').stat().st_ino == identity
                assert hashes(root / 'result') == expected
                assert hashes(root / 'published') == expected
            print('OUTER_COMMIT_DEATH_RETRY_PASS', stage, flush=True)
    finally:
        if child.poll() is None:
            signal.pidfd_send_signal(fd, signal.SIGKILL)
            child.wait(timeout=5)
        os.close(fd)
    shutil.rmtree(root)
assert hashes(source) == expected
assert not Path('/work/result').exists()
print('OUTER_COMMIT_DEATH_MATRIX_PASS', flush=True)
