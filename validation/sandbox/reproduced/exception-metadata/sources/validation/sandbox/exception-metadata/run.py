#!/usr/bin/python3
"""Record bounded userspace exception-metadata characterization; never assemble."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[3]
out = args.out.resolve()
out.mkdir()
helper = repo / 'validation/sandbox/publication/helper'
target = repo / 'validation/sandbox/copy/helper/target'
paths = [repo/'Cargo.toml', repo/'Cargo.lock', helper/'Cargo.toml', helper/'Cargo.lock',
         helper/'src/bin/exceptions.rs', Path(__file__).resolve()]
paths += list((repo/'src').rglob('*.rs'))
catalog = {}
for path in paths:
    relative = path.relative_to(repo)
    copied = out/'sources'/relative
    copied.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, copied)
    catalog[str(relative)] = hashlib.sha256(path.read_bytes()).hexdigest()
build = ['cargo', 'build', '--locked', '--manifest-path', str(helper/'Cargo.toml'),
         '--target-dir', str(target), '--bin', 'exceptions']
with (out/'build.log').open('wb') as log:
    subprocess.run(build, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=180)
command = [str(target/'debug/exceptions'), str(out/'fixture')]
with (out/'results.txt').open('wb') as log:
    subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=10)
lines = (out/'results.txt').read_text().splitlines()
for case in ('valid-control', 'terminator-old-ignored', 'metadata-as-data', 'data-past-store',
             'origin-past-64g', 'duplicate-origin', 'duplicate-data'):
    assert lines.count('EXCEPTION_METADATA_AUTHENTICATED_WITHOUT_SEMANTIC_CHECK '+case) == 1
assert lines.count('EXCEPTION_METADATA_USERSPACE_CHARACTERIZATION_COMPLETE') == 1
for relative, digest in catalog.items():
    assert hashlib.sha256((repo/relative).read_bytes()).hexdigest() == digest
(out/'inputs.json').write_text(json.dumps({'build':build, 'command':command,
    'euid':os.geteuid(), 'binary_sha256':hashlib.sha256((target/'debug/exceptions').read_bytes()).hexdigest(),
    'sources':catalog, 'scope':'userspace only; no assemble, device or mount operations'},indent=2)+'\n')
print('USERSPACE_EXCEPTION_METADATA_EVIDENCE_RECORDED', out)
