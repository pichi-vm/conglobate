#!/usr/bin/env python3
"""Relink trusted clean-pair objects with a diagnostic-only release wrapper."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--pair', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
pair, out = a.pair.resolve(), a.out.resolve()
out.mkdir()
source = Path(__file__).with_name('release-fault.c').resolve()
build = pair / 'worker-build'
commands = [
    ['gcc', '-I' + str(build / 'lib'), '-I' + str(pair / 'worker-source/lib'),
     '-c', str(source), '-o', str(out / 'release-fault.o')],
    ['gcc', '-pthread', '-Wl,--wrap=ext2fs_flush2', '-o', str(out / 'acl-patched'),
     str(out / 'release-fault.o'), 'fuse2fs.o', 'journal.o', 'recovery.o', 'revoke.o',
     '../lib/libext2fs.a', '../lib/libcom_err.a', '../lib/libsupport.a',
     str(pair / 'libfuse3.so.4'), '-lacl', '-lblkid', '-luuid',
     '../lib/libext2fs.a', '-lpthread', '../lib/libe2p.a', '../lib/libcom_err.a'],
]
inputs = [source, Path(__file__).resolve(), pair / 'libfuse3.so.4',
          *[build / 'misc' / n for n in ('fuse2fs.o', 'journal.o', 'recovery.o', 'revoke.o')],
          *[build / 'lib' / n for n in ('libext2fs.a', 'libcom_err.a', 'libsupport.a', 'libe2p.a')]]
def hashes():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
before = hashes()
for i, command in enumerate(commands):
    with (out / f'build-{i}.log').open('wb') as log:
        subprocess.run(command, cwd=build / 'misc', stdout=log, stderr=subprocess.STDOUT,
                       check=True, timeout=30)
assert before == hashes()
shutil.copy2(pair / 'libfuse3.so.4', out / 'libfuse3.so.4')
(out / 'build.json').write_text(json.dumps({'inputs': before, 'commands': commands,
    'cwd': str(build / 'misc'), 'outputs': {n: hashlib.sha256((out / n).read_bytes()).hexdigest()
                                         for n in ('acl-patched', 'libfuse3.so.4')}}, indent=2) + '\n')
print('DIAGNOSTIC_RELEASE_WORKER_BUILT')
