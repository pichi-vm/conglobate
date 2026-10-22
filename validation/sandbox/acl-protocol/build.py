#!/usr/bin/python3
"""Disposable paired-library ACL protocol experiment; installs nothing."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--fuse-source',type=Path,required=True)
p.add_argument('--worker-source',type=Path,required=True,help='finalize+acl-fix patched source tree')
p.add_argument('--meson-pythonpath',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args(); out=a.out.resolve();out.mkdir()
fuse=out/'fuse-source';worker=out/'worker-source'
shutil.copytree(a.fuse_source,fuse);shutil.copytree(a.worker_source,worker)
patches=[]
originals={str(path.relative_to(out)):hashlib.sha256(path.read_bytes()).hexdigest()
           for path in (fuse/'include/fuse.h',fuse/'lib/fuse_i.h',fuse/'lib/fuse_lowlevel.c',
                        fuse/'lib/fuse.c',fuse/'lib/fuse_versionscript',worker/'misc/fuse2fs.c')}
(out/'original-inputs.json').write_text(json.dumps(originals,indent=2)+'\n')
def patch(path,old,new):
 text=path.read_text();assert text.count(old)==1,(path,old)
 changed=text.replace(old,new);path.write_text(changed)
 patches.append(''.join(difflib.unified_diff(text.splitlines(True),changed.splitlines(True),fromfile='a/'+str(path.relative_to(out)),tofile='b/'+str(path.relative_to(out)))))
# Keep public context layouts unchanged. A prototype-only exported accessor
# makes an accidentally mixed stock library fail to load, not misread memory.
patch(fuse/'lib/fuse_i.h','\tstruct fuse_ctx ctx;',
      '\tstruct fuse_ctx ctx;\n\tunsigned int probe_setxattr_flags;')
patch(fuse/'lib/fuse_lowlevel.c','\t/* XXX:The API should be extended to support extra_flags/setxattr_flags */',
      '\treq->probe_setxattr_flags = (req->se->conn.want & FUSE_CAP_SETXATTR_EXT) ? arg->setxattr_flags : 0;')
patch(fuse/'include/fuse.h','struct fuse_context *fuse_get_context(void);',
      'struct fuse_context *fuse_get_context(void);\n/* Experimental: only valid during the high-level setxattr callback. */\nunsigned int fuse_probe_setxattr_flags(void);')
patch(fuse/'lib/fuse.c','struct fuse_context *fuse_get_context(void)',
      '/* Disposable protocol accessor, not an adopted public API. */\nunsigned int fuse_probe_setxattr_flags(void) {\n struct fuse_context_i *c = fuse_get_context_internal();\n return c && c->req ? c->req->probe_setxattr_flags : 0;\n}\n\nstruct fuse_context *fuse_get_context(void)')
patch(fuse/'lib/fuse_versionscript','# Local Variables:',
      'FUSE_PROBE_1.0 { global: fuse_probe_setxattr_flags; } FUSE_3.18;\n\n# Local Variables:')
code=worker/'misc/fuse2fs.c'
old='''\tif (setting_acl && !is_superuser(ff, fuse_get_context())) {
\t\tret = in_file_group(fuse_get_context(), &inode);
\t\tif (ret < 0)
\t\t\treturn ret;
\t\tif (!ret)
\t\t\tinode.i_mode &= ~S_ISGID;
\t}'''
patch(code,old,'''\t/* Protocol bit FUSE_SETXATTR_ACL_KILL_SGID, from the kernel. */
\tif (setting_acl && (fuse_probe_setxattr_flags() & 1U))
\t\tinode.i_mode &= ~S_ISGID;''')
patch(code,'\t\tconn->want |= FUSE_CAP_POSIX_ACL | FUSE_CAP_DONT_MASK;',
      '\t\tconn->want |= FUSE_CAP_POSIX_ACL | FUSE_CAP_DONT_MASK | FUSE_CAP_SETXATTR_EXT;')
patch(code,'\t\t/* fuse started up enough to call op_init */\n\t\tret = 0;',
      '\t\t/* Nonzero FUSE loop/initialization status is a mount failure. */\n\t\tret = 32;')
(out/'protocol.patch').write_text(''.join(patches))
env=os.environ|{'PYTHONPATH':str(a.meson_pythonpath.resolve())}
def run(command,log,cwd=None,environment=env):
 with (out/log).open('wb') as stream:subprocess.run(command,cwd=cwd,env=environment,stdout=stream,stderr=subprocess.STDOUT,check=True)
run(['python3','-m','mesonbuild.mesonmain','setup',str(out/'fuse-build'),str(fuse),'-Dtests=false','-Dexamples=false','-Dutils=false','-Denable-io-uring=false'],'fuse-configure.log')
run(['ninja','-C',str(out/'fuse-build')],'fuse-build.log')
lib=(out/'fuse-build/lib/libfuse3.so.4').resolve()
shutil.copy2(lib,out/'libfuse3.so.4')
build=out/'worker-build';build.mkdir()
env=env|{'fuse3_CFLAGS':f'-I{fuse / "include"} -I{out / "fuse-build"}', 'fuse3_LIBS':str(lib)+' -lacl'}
run([str(worker/'configure'),'--disable-nls','--disable-elf-shlibs','--disable-fsck','--disable-e2initrd-helper','--enable-fuse2fs'],'worker-configure.log',build,env)
run(['make','-j8','libs'],'worker-libs.log',build,env)
run(['make','-C','misc','-j8','fuse2fs'],'worker-build.log',build,env)
shutil.copy2(build/'misc/fuse2fs',out/'acl-patched')
paths=[out/'acl-patched',out/'libfuse3.so.4',out/'protocol.patch',Path(__file__).resolve(),code,fuse/'lib/fuse.c',fuse/'lib/fuse_lowlevel.c',fuse/'lib/fuse_i.h',fuse/'include/fuse.h',fuse/'lib/fuse_versionscript']
(out/'build-inputs.json').write_text(json.dumps({str(x):hashlib.sha256(x.read_bytes()).hexdigest() for x in paths},indent=2)+'\n')
print('ACL_PROTOCOL_BUILD_PASS',flush=True)
