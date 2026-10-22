#!/usr/bin/env python3
"""Reuse the disposable guest packager; no host mounts or policy changes."""
import importlib.util, pathlib, sys, subprocess, json, hashlib, shutil
here=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('guest',here.parent/'guest.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
out=pathlib.Path(sys.argv[1]).resolve()
worker=pathlib.Path(sys.argv[2] if len(sys.argv)>2 else '/tmp/conglobate-acl-protocol-v4').resolve()
fixed='--fixed' in sys.argv
g.CASES['deployment']={'init':'worker-placement/init','placement.py':'worker-placement/probe.py'}
if fixed: g.CASES['deployment']['expect-placement-fixed']='worker-placement-fix/expected'
sys.argv=['guest.py','deployment','--kernel-version','7.2.10-200.fc44.x86_64','--out',str(out)]
g.main()
root=out/'root'
shutil.copy2(worker/'acl-patched',root/'worker')
shutil.copy2(worker/'libfuse3.so.4',root/'usr/lib64/libfuse3.so.4')
# Dependencies other than the paired libfuse are supplied by the existing fuse2fs fixture.
ldd=subprocess.check_output(['ldd',str(worker/'acl-patched')],text=True,env={'LD_LIBRARY_PATH':str(worker),'PATH':'/usr/bin:/bin'})
(out/'worker-ldd.txt').write_text(ldd)
assert 'not found' not in ldd
inventory=json.loads((out/'inputs.json').read_text())
for p in [root/'worker',root/'usr/lib64/libfuse3.so.4']:
 inventory['files'][str(p.relative_to(out))]=hashlib.sha256(p.read_bytes()).hexdigest()
(out/'inputs.json').write_text(json.dumps(inventory,indent=2)+'\n')
paths=[str(p.relative_to(root)) for p in sorted(root.rglob('*'))]
a=subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=b'\0'.join(p.encode() for p in paths)+b'\0',cwd=root,capture_output=True,check=True)
with (out/'initramfs.gz').open('wb') as f: subprocess.run(['gzip','-1','-n'],input=a.stdout,stdout=f,check=True)
cmd=json.loads((out/'command.json').read_text())
with (out/'results.txt').open('wb') as f: subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=95,check=True)
s=(out/'results.txt').read_text();g.validate_guest_exit(s)
assert 'PLACEMENT_MATRIX_COMPLETE' in s
print(s[s.find('PLACEMENT_BEGIN'):])
