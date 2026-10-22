#!/usr/bin/env python3
import pathlib as pl,shutil,subprocess,json,gzip,hashlib,sys
P=pl.Path;here=P(__file__).resolve().parent;legacy=here.parent/'sandbox/kernel-mount-lsm';seed=P('/var/tmp/conglobate-cap-review-fix-normal-v1');out=P(sys.argv[1]);out.mkdir();root=out/'root';shutil.copytree(seed/'root',root,symlinks=True)
s=(seed/'root/matrix.py').read_text().split("if P('/runtimes').read_text().strip()=='legacy':")[0]
s=s.replace('magic==0x65735546','magic==0xef53').replace('FUSE_FILESYSTEM_VERIFIED','KERNEL_EXT4_VERIFIED')
s=s.replace("mounts=[('/rpms'", "mounts=[('/work/other','/other',False),('/rpms'")
s+='\n'+(legacy/'guest.py').read_text()
if '--two-targets' in sys.argv:
 s=s.replace("str(sbns)])", "str(sbns),str(os.stat(othernative).st_dev)])")
(root/'matrix.py').write_text(s)
s=(root/'probe.py').read_text().replace('("lsm-off","lsm-foreign")','("lsm-off","lsm-off-after","lsm-foreign")')
s=s.replace(" other='/tmp/lsm-negative'", " otherfile='/other/negative';P(otherfile).write_text('ordinary write works')\n must_errno(lambda:os.setxattr(otherfile,'security.selinux',LABEL),errno.EPERM)\n must_errno(lambda:os.removexattr(otherfile,'security.selinux'),errno.EPERM)\n emit('OTHER_KERNEL_FILESYSTEM_DENIED')\n ctypes.set_errno(0);assert libc.unshare(0)==-1 and ctypes.get_errno()==errno.EPERM\n emit('COMMAND_NAMESPACE_RESTRICTION_PASS')\n other='/tmp/lsm-negative'" )
if '--two-targets' in sys.argv:
 s=s.replace("must_errno(lambda:os.setxattr(otherfile,'security.selinux',LABEL),errno.EPERM)", "os.setxattr(otherfile,'security.selinux',LABEL);assert os.getxattr(otherfile,'security.selinux')==LABEL")
 s=s.replace("must_errno(lambda:os.removexattr(otherfile,'security.selinux'),errno.EPERM)", "os.removexattr(otherfile,'security.selinux')")
 s=s.replace('OTHER_KERNEL_FILESYSTEM_DENIED','SECOND_ASSIGNED_FILESYSTEM_LABEL_PASS')
(root/'probe.py').write_text(s)
if '--failure' in sys.argv:(root/'failure-mode').touch()

include='/var/tmp/conglobate-cap-review-bpf/sysroot/usr/include'
policy=here.parent.parent/'policy/labels.bpf.c'
subprocess.run(['clang','-O2','-g','-Wall','-Werror','-target','bpf','-I'+include,'-c',str(policy),'-o',str(root/'label-policy.bpf.o')],check=True)
subprocess.run(['cc','-O2','-Wall','-Wextra','-Werror','-Wno-unused-parameter','-I'+include,str(here/'loader.c'),'/usr/lib64/libbpf.so.1','-o',str(root/'kernel-label-loader')],check=True)
shutil.copy2(policy,out/'labels.bpf.c');shutil.copy2(here/'loader.c',out/'loader.c')
shutil.copy2(seed/'vmlinuz',out/'vmlinuz')
paths=sorted(root.rglob('*'));(out/'inputs.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.is_file() and not p.is_symlink()},indent=2))
r=subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=b'\0'.join(str(p.relative_to(root)).encode() for p in paths)+b'\0',cwd=root,capture_output=True,check=True)
(out/'initramfs.gz').write_bytes(gzip.compress(r.stdout,compresslevel=1,mtime=0))
cmd=[v.replace(str(seed),str(out)) for v in json.loads((seed/'command.json').read_text())];(out/'command.json').write_text(json.dumps(cmd))
with (out/'results.txt').open('wb') as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=300,check=True)
s=(out/'results.txt').read_text();assert 'KERNEL_MOUNT_LSM_PASS' in s and 'GUEST_EXIT 0' in s,s[-5000:]
if '--two-targets' in sys.argv:assert 'SECOND_ASSIGNED_FILESYSTEM_LABEL_PASS' in s
(out/'verdict.json').write_text(json.dumps({'kernel_mount_lsm':True,'production_qualified':False}));print('PASS',out)
