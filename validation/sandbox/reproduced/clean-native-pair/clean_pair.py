#!/usr/bin/env python3
"""Build the experimental pair from release archives in a new owned directory.

Trusted source/build tools only. Installs nothing; runs no image command.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--archives', type=Path, required=True)
p.add_argument('--finalize', type=Path, required=True)
p.add_argument('--pair', type=Path, required=True)
p.add_argument('--meson-pythonpath', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
out = a.out.resolve()
out.mkdir()
(out / 'home').mkdir()
env = {'PATH': '/usr/bin:/bin', 'HOME': str(out / 'home'), 'LC_ALL': 'C',
       'PYTHONPATH': str(a.meson_pythonpath.resolve())}
commands = []
inputs = {}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, log, cwd=out, environment=env):
    commands.append({'argv': command, 'cwd': str(cwd), 'environment': environment})
    (out / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
    with (out / log).open('wb') as stream:
        subprocess.run(command, cwd=cwd, env=environment, stdout=stream,
                       stderr=subprocess.STDOUT, check=True, timeout=240)


for archive, name, dest in [
    (a.archives / 'e2fs/e2fsprogs-1.47.3.tar.xz', 'e2fsprogs-1.47.3', 'worker-source'),
    (a.archives / 'fuse/fuse-3.18.3.tar.gz', 'fuse-3.18.3', 'fuse-source'),
]:
    inputs[str(archive.resolve())] = sha(archive)
    with tarfile.open(archive) as stream:
        stream.extractall(out, filter='data')
    (out / name).rename(out / dest)

worker, fuse = out / 'worker-source', out / 'fuse-source'
for patch, cwd in [(a.finalize / 'finalization.patch', worker),
                   (a.finalize / 'acl.patch', worker),
                   (a.pair / 'protocol.patch', out)]:
    inputs[str(patch.resolve())] = sha(patch)
    run(['git', 'apply', '--check', str(patch.resolve())], patch.name + '.check.log', cwd)
    run(['git', 'apply', str(patch.resolve())], patch.name + '.log', cwd)
inputs[str(Path(__file__).resolve())] = sha(Path(__file__))
(out / 'input-hashes.json').write_text(json.dumps(inputs, indent=2) + '\n')

# Regenerate configure and config.h.in; shipped aclocal.m4 and auxiliary scripts
# remain explicit release inputs. No prior build/configuration tree is copied.
generated = {}
for name in ('configure', 'lib/config.h.in'):
    generated[name] = {'release_sha256': sha(worker / name)}
    (worker / name).unlink()
run(['autoconf'], 'autoconf.log', worker)
run(['autoheader'], 'autoheader.log', worker)
for name in generated:
    generated[name]['regenerated_sha256'] = sha(worker / name)
(out / 'regenerated.json').write_text(json.dumps(generated, indent=2) + '\n')

run(['python3', '-m', 'mesonbuild.mesonmain', 'setup', str(out / 'fuse-build'),
     str(fuse), '-Dtests=false', '-Dexamples=false', '-Dutils=false',
     '-Denable-io-uring=false'], 'fuse-configure.log')
run(['ninja', '-C', str(out / 'fuse-build')], 'fuse-build.log')
library = (out / 'fuse-build/lib/libfuse3.so.4').resolve()
shutil.copy2(library, out / 'libfuse3.so.4')
build = out / 'worker-build'
build.mkdir()
worker_env = env | {'fuse3_CFLAGS': f'-I{fuse / "include"} -I{out / "fuse-build"}',
                    'fuse3_LIBS': str(library) + ' -lacl'}
run([str(worker / 'configure'), '--disable-nls', '--disable-elf-shlibs',
     '--disable-fsck', '--disable-e2initrd-helper', '--enable-fuse2fs'],
    'worker-configure.log', build, worker_env)
run(['make', '-j8', 'libs'], 'worker-libs.log', build, worker_env)
run(['make', '-C', 'misc', '-j8', 'fuse2fs'], 'worker-build.log', build, worker_env)
shutil.copy2(build / 'misc/fuse2fs', out / 'acl-patched')
report = {'outputs': {n: sha(out / n) for n in ('acl-patched', 'libfuse3.so.4')},
          'worker_source_matches_prior_pair': sha(worker / 'misc/fuse2fs.c') ==
              sha(a.pair / 'worker-source/misc/fuse2fs.c'),
          'limits': ['Host compiler, headers, tools and Meson module are provisioned inputs.',
                     'Release aclocal.m4 and auxiliary scripts are not bootstrapped.',
                     'Fresh-path binaries need new behavior validation; byte identity is not claimed.']}
assert report['worker_source_matches_prior_pair']
(out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
print('CLEAN_PAIR_BUILD_PASS', flush=True)
