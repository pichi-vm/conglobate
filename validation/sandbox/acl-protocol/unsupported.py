#!/usr/bin/python3
"""Build a diagnostic library that hides one negotiated capability."""
import argparse,difflib,hashlib,json,os,shutil,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--pair-build',type=Path,required=True)
p.add_argument('--meson-pythonpath',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args();out=a.out.resolve();out.mkdir()
source=out/'source';shutil.copytree(a.pair_build/'fuse-source',source)
path=source/'lib/fuse_lowlevel.c';before=path.read_text()
old='\tse->conn.time_gran = 1;'
assert before.count(old)==1
new='\t/* Diagnostic only: simulate a peer lacking SETXATTR_EXT. */\n\tse->conn.capable_ext &= ~FUSE_CAP_SETXATTR_EXT;\n'+old
after=before.replace(old,new);path.write_text(after)
(out/'unsupported.patch').write_text(''.join(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/lib/fuse_lowlevel.c',tofile='b/lib/fuse_lowlevel.c')))
env=os.environ|{'PYTHONPATH':str(a.meson_pythonpath.resolve())}
for command,log in [(['python3','-m','mesonbuild.mesonmain','setup',str(out/'build'),str(source),'-Dtests=false','-Dexamples=false','-Dutils=false','-Denable-io-uring=false'],'configure.log'),(['ninja','-C',str(out/'build')],'build.log')]:
 with (out/log).open('wb') as f:subprocess.run(command,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
shutil.copy2(out/'build/lib/libfuse3.so.4',out/'libfuse3.so.4')
shutil.copy2(a.pair_build/'acl-patched',out/'acl-patched')
paths=[path,out/'libfuse3.so.4',out/'acl-patched',out/'unsupported.patch',Path(__file__).resolve()]
(out/'build-inputs.json').write_text(json.dumps({str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},indent=2)+'\n')
print('UNSUPPORTED_EXTENSION_LIBRARY_BUILT')
