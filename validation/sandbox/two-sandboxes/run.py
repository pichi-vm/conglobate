#!/usr/bin/env python3
"""Reuse the disposable guest packager; no host mounts or policy changes."""
import importlib.util, pathlib, sys, subprocess, json, hashlib, shutil
here=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('guest',here.parent/'guest.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
out=pathlib.Path(sys.argv[1]).resolve()
worker=pathlib.Path('/var/tmp/conglobate-placement-fixed-v2')
fixed='--fixed' in sys.argv
g.CASES['deployment']={'init':'two-sandboxes/init','topology.py':'two-sandboxes/probe.py','checks.py':'worker-placement/probe.py','expect-placement-fixed':'worker-placement-fix/expected'}

sys.argv=['guest.py','deployment','--kernel-version','7.2.10-200.fc44.x86_64','--out',str(out)]
g.main()
root=out/'root'
# Import only the already-tested metadata helper definitions.
f=root/'checks.py';f.write_text(f.read_text().split("if len(sys.argv)>1:")[0].replace("['--bounding-set=-fsetid']","['--bounding-set=-fsetid','--inh-caps=-fsetid','--ambient-caps=-fsetid']"))
shutil.copy2('/usr/bin/bwrap',root/'usr/bin/bwrap')
import re
for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd','/usr/bin/bwrap'],text=True)):
 p=pathlib.Path(lib)
 if p.is_file():
  dest=root/str(p).lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,dest)
shutil.copy2(worker/'acl-patched',root/'usr/bin/fuse-worker')
shutil.copy2(worker/'libfuse3.so.4',root/'usr/lib64/libfuse3.so.4')
# Dependencies other than the paired libfuse are supplied by the existing fuse2fs fixture.
ldd=subprocess.check_output(['ldd',str(worker/'acl-patched')],text=True,env={'LD_LIBRARY_PATH':str(worker),'PATH':'/usr/bin:/bin'})
(out/'worker-ldd.txt').write_text(ldd)
assert 'not found' not in ldd
inventory=json.loads((out/'inputs.json').read_text())
for p in root.rglob('*'):
 if not p.is_file() or p.is_symlink(): continue
 inventory['files'][str(p.relative_to(out))]=hashlib.sha256(p.read_bytes()).hexdigest()
(out/'inputs.json').write_text(json.dumps(inventory,indent=2)+'\n')
paths=[str(p.relative_to(root)) for p in sorted(root.rglob('*'))]
a=subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=b'\0'.join(p.encode() for p in paths)+b'\0',cwd=root,capture_output=True,check=True)
with (out/'initramfs.gz').open('wb') as f: subprocess.run(['gzip','-1','-n'],input=a.stdout,stdout=f,check=True)
cmd=json.loads((out/'command.json').read_text())
with (out/'results.txt').open('wb') as f: subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=120,check=True)
s=(out/'results.txt').read_text();g.validate_guest_exit(s)
assert 'TWO_SANDBOX_MATRIX_COMPLETE' in s
assert 'NO_SANDBOX_PROCESSES_REMAIN' in s
assert s.count('TOPOLOGY_CASE_PASS ')==5
assert s.count('CHMOD_PERMISSION_MATRIX_PASS')==5
outer=json.loads(next(l.split(' ',1)[1] for l in s.splitlines() if l.startswith('SUPERVISOR_ENDPOINT_CONTROLS_PASS ')))
roles={r:[json.loads(l.split(' ',2)[2]) for l in s.splitlines() if l.startswith('ISOLATION_PASS '+r+' ')] for r in ('worker','command')}
assert len(roles['worker'])==len(roles['command'])==5
for w,c in zip(roles['worker'],roles['command']):
 for key in outer: assert len({outer[key],w[key],c[key]})==3,(key,w,c)
(out/'verdict.json').write_text(json.dumps({'topology_cases':5,'separate_namespaces':['user','mnt','pid','net','ipc','uts'],'metadata_cases':5,'full_production_sandbox_qualified':False},indent=2)+'\n')
print(s[s.find('TOPOLOGY_BEGIN'):])
