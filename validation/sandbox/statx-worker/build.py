#!/usr/bin/env python3
"""Fresh worker build with experimental statx, retaining the existing ACL pair."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--pair', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
pair, out = args.pair.resolve(), args.out.resolve()
out.mkdir()
worker = out / 'worker-source'
shutil.copytree(pair / 'worker-source', worker)
source = worker / 'misc/fuse2fs.c'
original = source.read_text()
text = original.replace('#include <sys/ioctl.h>', '#include <sys/ioctl.h>\n#include <sys/sysmacros.h>')
old = 'static int stat_inode(ext2_filsys fs, ext2_ino_t ino, struct stat *statbuf)'
assert text.count(old) == 1
text = text.replace(old, 'static int stat_inode_birth(ext2_filsys fs, ext2_ino_t ino, struct stat *statbuf, struct timespec *birth, int *has_birth)')
needle = '\tmemcpy(&fakedev, fs->super->s_uuid, sizeof(fakedev));'
assert text.count(needle) == 1
text = text.replace(needle, '''\tif (birth && has_birth) {
        *has_birth = EXT4_FITS_IN_INODE(&inode, i_crtime);
        if (*has_birth)
            EXT4_EINODE_GET_XTIME(i_crtime, birth, (&inode));
    }
''' + needle)
needle = 'static int op_getattr('
assert text.count(needle) == 1
wrapper = '''static int stat_inode(ext2_filsys fs, ext2_ino_t ino, struct stat *st)
{ return stat_inode_birth(fs, ino, st, NULL, NULL); }

'''
callback = Path(__file__).with_name('callback.c').read_text()
text = text.replace(needle, wrapper + callback + '\n' + needle)
text = text.replace('\t.getattr = op_getattr,', '\t.getattr = op_getattr,\n\t.statx = op_statx,')
source.write_text(text)
(out / 'statx.patch').write_text(''.join(difflib.unified_diff(original.splitlines(True), text.splitlines(True), fromfile='a/misc/fuse2fs.c', tofile='b/misc/fuse2fs.c')))
build = out / 'worker-build'
build.mkdir()
lib = (pair / 'fuse-build/lib/libfuse3.so.4').resolve()
env = os.environ | {'fuse3_CFLAGS': f'-I{pair / "fuse-source/include"} -I{pair / "fuse-build"}', 'fuse3_LIBS': str(lib) + ' -lacl'}
commands = [[str(worker / 'configure'), '--disable-nls', '--disable-elf-shlibs', '--disable-fsck', '--disable-e2initrd-helper', '--enable-fuse2fs'], ['make', '-j8', 'libs'], ['make', '-C', 'misc', '-j8', 'fuse2fs']]
for index, command in enumerate(commands):
    with (out / f'build-{index}.log').open('wb') as log:
        subprocess.run(command, cwd=build, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
shutil.copy2(build / 'misc/fuse2fs', out / 'acl-patched')
shutil.copy2(lib, out / 'libfuse3.so.4')
paths = [source, pair / 'worker-source/misc/fuse2fs.c', out / 'acl-patched', out / 'libfuse3.so.4', out / 'statx.patch', Path(__file__).resolve(), Path(__file__).with_name('callback.c').resolve()]
(out / 'build-inputs.json').write_text(json.dumps({str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}, indent=2) + '\n')
print('STATX_WORKER_BUILD_PASS')
