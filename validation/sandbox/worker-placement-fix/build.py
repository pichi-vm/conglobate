#!/usr/bin/env python3
"""Build an isolated fuse2fs correction; keep the paired libfuse unchanged."""
import argparse,difflib,hashlib,json,os,pathlib,shutil,subprocess
p=argparse.ArgumentParser();p.add_argument('--pair',type=pathlib.Path,required=True);p.add_argument('--out',type=pathlib.Path,required=True);a=p.parse_args()
pair=a.pair.resolve();out=a.out.resolve();out.mkdir()
worker=out/'worker-source';shutil.copytree(pair/'worker-source',worker)
f=worker/'misc/fuse2fs.c';before=f.read_text()
old='''\tif (!is_superuser(ff, ctxt)) {
\t\tret = in_file_group(ctxt, &inode);'''
new='''\t/* With -o kernel, default_permissions makes the kernel's
\t * setattr_prepare() validate ownership and clear S_ISGID when needed.
\t * Honor that mode instead of rechecking supplementary groups through
\t * /proc, whose IDs need not use the FUSE connection's user namespace.
\t * Retain userspace permission checks for non-kernel mode.
\t */
\tif (!ff->kernel && !is_superuser(ff, ctxt)) {
\t\tret = in_file_group(ctxt, &inode);'''
assert before.count(old)==1
f.write_text(before.replace(old,new))
(out/'placement.patch').write_text(''.join(difflib.unified_diff(before.splitlines(True),f.read_text().splitlines(True),fromfile='a/misc/fuse2fs.c',tofile='b/misc/fuse2fs.c')))
lib=(pair/'fuse-build/lib/libfuse3.so.4').resolve()
(out/'tmp').mkdir()
env=os.environ|{'TMPDIR':str(out/'tmp'),'fuse3_CFLAGS':f'-I{pair/"fuse-source/include"} -I{pair/"fuse-build"}','fuse3_LIBS':str(lib)+' -lacl'}
build=out/'worker-build';build.mkdir()
commands=[[str(worker/'configure'),'--disable-nls','--disable-elf-shlibs','--disable-fsck','--disable-e2initrd-helper','--enable-fuse2fs'],['make','-j8','libs'],['make','-C','misc','-j8','fuse2fs']]
for i,c in enumerate(commands):
 with (out/f'build-{i}.log').open('wb') as log:subprocess.run(c,cwd=build,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
shutil.copy2(build/'misc/fuse2fs',out/'acl-patched');shutil.copy2(lib,out/'libfuse3.so.4')
files=[f,pair/'worker-source/misc/fuse2fs.c',out/'acl-patched',out/'libfuse3.so.4',out/'placement.patch',pathlib.Path(__file__).resolve()]
(out/'build-inputs.json').write_text(json.dumps({str(x):hashlib.sha256(x.read_bytes()).hexdigest() for x in files},indent=2)+'\n')
print('PLACEMENT_WORKER_BUILD_PASS')
