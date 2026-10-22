#!/usr/bin/python3
"""Fixture publisher: commit only after successful init and resource checks."""
import hashlib
from pathlib import Path
import subprocess
private = Path('/work/published')
public = Path('/work/result')
assert private.joinpath('inventory.cbor').is_file()
assert not public.exists()
status = int(Path('/work/owner-status').read_text())
mode = Path('/completion-mode').read_text().strip()
def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}
before = hashes(private)
if mode == 'late-failure':
    assert status == 17, status
    assert not public.exists()
    assert hashes(private) == before
    print('OUTER_COMMIT_LATE_FAILURE_NO_PUBLIC_OUTPUT_PASS', flush=True)
else:
    assert mode == 'success' and status == 0, (mode, status)
    subprocess.run(['/commit-output'], check=True)
    assert not private.exists()
    assert hashes(public) == before
    print('OUTER_COMMIT_SUCCESS_BYTES_PRESERVED_PASS', flush=True)
