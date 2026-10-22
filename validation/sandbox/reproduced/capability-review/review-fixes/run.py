#!/usr/bin/env python3
"""Package benign mechanism probes inside the existing no-NIC disposable VM."""
import argparse,gzip,hashlib,json,pathlib,shutil,subprocess,re
P=pathlib.Path
p=argparse.ArgumentParser();p.add_argument('out',type=P);p.add_argument('--cleanup-failure',action='store_true');p.add_argument('--kill-loader',action='store_true');p.add_argument('--packages',action='store_true');p.add_argument('--lsm',action='store_true');p.add_argument('--seed',type=P,default='/var/tmp/conglobate-runtime-pass-v24');a=p.parse_args()
if a.cleanup_failure and (not a.lsm or a.packages):p.error('--cleanup-failure requires --lsm and excludes --packages')
here=P(__file__).resolve().parent;out=a.out.resolve();out.mkdir();root=out/'root';shutil.copytree(a.seed/'root',root,symlinks=True)
for executable in ([] if not a.lsm else ['/var/tmp/conglobate-cap-review-bpf/label-loader']):
 target=root/executable.lstrip('/');target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(executable,target)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',executable],text=True)):
  if P(lib).is_file():
   target=root/lib.lstrip('/');target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,target)
if a.packages:
 package_seed=P('/tmp/conglobate-rebuilt-package-seed-v2/root')
 for f in (package_seed/'usr').rglob('*'):
  target=root/f.relative_to(package_seed)
  if target.exists() or target.is_symlink():continue
  if f.is_symlink():target.parent.mkdir(parents=True,exist_ok=True);target.symlink_to(f.readlink())
  elif f.is_dir():target.mkdir(parents=True,exist_ok=True)
  else:target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,target)
 shutil.copytree(package_seed/'rpms',root/'rpms');shutil.copytree(package_seed/'etc',root/'pkg-etc',symlinks=True)
source=(here.parent/'runtime-validation/matrix.py').read_text()
source=source.replace(" assert os.getuid()"," if role=='command':\n  import probe;probe.clamp_bounding()\n assert os.getuid()")
source=source.replace(" filter_probe()\n emit('ROLE_READY'", " if role=='worker':filter_probe()\n emit('ROLE_READY'")
source=source.replace(" # A child with SYS_ADMIN removed", " import probe;probe.run();emit('METADATA_PASS');return\n # A child with SYS_ADMIN removed")
source=source.replace("for case in ('normal','exit42','descendant','launcher-death','worker-death','timeout','helper-death'):","for case in ('normal',):")
source=source.replace("mounts=[('/usr','/usr',True),", "mounts=[('/probe.py','/probe.py',True),('/usr','/usr',True),")
if a.packages:
 source=source.replace("'-s','64M'","'-s','2G'").replace('cp.wait(timeout=3)','cp.wait(timeout=180)')
 source=source.replace("mounts=[('/probe.py'","mounts=[('/rpms','/rpms',True),('/pkg-etc','/etc',True),('/probe.py'")
if a.lsm:
 source=source.replace("s.sendmsg([b'tree']", "s.sendmsg([json.dumps({'dev':os.stat(view).st_dev}).encode()]").replace("assert data==b'tree';tree", "mountdev=json.loads(data)['dev'];tree")
 source=source.replace("'SYS_ADMIN',",'').replace('00000000802401db','00000000800401db').replace('probe.clamp_bounding()','probe.clamp_bounding(False)').replace('probe.run()','probe.run_lsm(case)')
 source=source.replace('wp=cp=None;files=[];logs=[];extra=None;fd=None','wp=cp=None;files=[];logs=[];extra=None;fd=None;policy=None;ns_hold=None')
 source=source.replace("os.close(tree)\n    else:cp=p", "os.close(tree)\n     P('/work/policy-bpf').mkdir()\n     call('mount','-t','bpf','bpf','/work/policy-bpf')\n     ns_hold=os.open(f'/proc/{owner}/ns/user',os.O_RDONLY)\n     namespace=int(os.readlink(f'/proc/{owner}/ns/user').split('[')[1][:-1])\n     policy=subprocess.Popen(['/var/tmp/conglobate-cap-review-bpf/label-loader','/label-policy.bpf.o',str(namespace),str(mountdev),'/work/policy-ready','/work/policy-bpf'])\n     for _ in range(500):\n      if P('/work/policy-ready').exists():break\n      assert policy.poll() is None,f'policy loader failed: {policy.returncode}'\n      time.sleep(.01)\n     assert P('/work/policy-ready').exists()\n    else:cp=p")
 # Quiesce commands before any mount, identity or policy teardown. If the
 # deadline fails, leave policy pinned and let the disposable VM terminate.
 source=source.replace('  finally:\n   for lp in logs:', """  finally:
   if policy is not None:
    command_records=[r for lp in logs for r in roles(lp) if r['role']=='command']
    if cp is not None and cp.poll() is None and not command_records:
     raise RuntimeError('Cannot prove command scope: retain policy until VM exit')
    deadline=time.monotonic()+5
    while live_in(command_records):
     for pid in live_in(command_records):
      try:os.kill(pid,signal.SIGKILL)
      except ProcessLookupError:pass
     if time.monotonic()>deadline:raise RuntimeError('Command quiescence failed: retain policy')
     time.sleep(.01)
    if cp is not None:cp.wait(timeout=2)
    assert not live_in(command_records)
    emit('COMMANDS_QUIESCENT_BEFORE_POLICY_CLEAR')
    if policy.poll() is None:policy.terminate();policy.wait(timeout=3)
    call('/var/tmp/conglobate-cap-review-bpf/label-loader','--clear','/work/policy-bpf');policy=None
    call('umount','/work/policy-bpf')
    emit('FAILURE_POLICY_CLEARED_AFTER_QUIESCENCE')
   for lp in logs:""")
 if a.cleanup_failure:
  source=source.replace("for case in ('normal',):", "for case in ('cleanup-failure',):")
  source=source.replace("   try:rc=cp.wait(timeout=", "   wait_marker(cp,logs[1],'CLEANUP_FAILURE_READY')\n   assert len(live_in(roles(logs[1])))>=2\n   emit('LIVE_COMMAND_AND_DESCENDANT_BEFORE_FAILURE')\n   raise RuntimeError('INJECTED_CLEANUP_FAILURE')\n   try:rc=cp.wait(timeout=")
  source=source.replace("   emit('RUNTIME_RESULT',result);results.append(result)", "   if result.get('error')=='INJECTED_CLEANUP_FAILURE' and not result.get('cleanup_failed'):result['status']='PASS';result['expected_failure']=True\n   emit('RUNTIME_RESULT',result);results.append(result)")

 if a.kill_loader:source=source.replace("assert P('/work/policy-ready').exists()\n    else:cp=p","assert P('/work/policy-ready').exists()\n     policy.kill();policy.wait(timeout=3);emit('POLICY_LOADER_KILLED_PINS_RETAINED')\n    else:cp=p")
 source=source.replace("    call('umount',exported);a.send(b'unmount')", """    foreign=subprocess.Popen(['nsenter','--user='+f'/proc/{owner}/ns/user','unshare','--user','--map-root-user','python3','-u','-c','import os,time; print(os.getpid(),flush=True); time.sleep(20)'],stdout=subprocess.PIPE,text=True)
    foreign_pid=int(foreign.stdout.readline())
    foreignlog=P('/work')/(ident+'-foreign-userns.log');logs.append(foreignlog)
    try:
     with foreignlog.open('w') as f:
      cforeign=subprocess.Popen(['python3','/matrix.py','launch',runtime,'command','lsm-foreign',image,exported,str(foreign_pid),ident+'-foreign','-1'],stdin=subprocess.DEVNULL,stdout=f,stderr=f)
      assert cforeign.wait(timeout=10)==0
     assert 'LSM_OTHER_NAMESPACE_DENIED' in foreignlog.read_text()
    finally:
     foreign.kill();foreign.wait(timeout=3)
    emit('OTHER_USER_NAMESPACE_CONTROL_PASS')
    if policy.poll() is None:policy.terminate();policy.wait(timeout=3)
    call('/var/tmp/conglobate-cap-review-bpf/label-loader','--clear','/work/policy-bpf');policy=None
    call('umount','/work/policy-bpf')
    offlog=P('/work')/(ident+'-policy-off.log');logs.append(offlog)
    with offlog.open('w') as f:
     off=subprocess.Popen(['python3','/matrix.py','launch',runtime,'command','lsm-off',image,exported,str(owner),ident+'-policy-off','-1'],stdin=subprocess.DEVNULL,stdout=f,stderr=f)
     assert off.wait(timeout=15)==0
    assert 'LSM_POLICY_REMOVAL_DENIES' in offlog.read_text()
    emit('POLICY_REMOVAL_CONTROL_PASS')
    call('umount',exported);a.send(b'unmount')""")
 source=source.replace('   for f in files:f.close()', '   if ns_hold is not None:os.close(ns_hold)\n   for f in files:f.close()')
 shutil.copy2('/var/tmp/conglobate-cap-review-bpf/label-policy.bpf.o',root/'label-policy.bpf.o')
source=source.replace("result['fsck']=call('e2fsck','-fn',image).splitlines()[-1]", """result['fsck']=call('e2fsck','-fn',image).splitlines()[-1]
    native='/work/native-check';P(native).mkdir();call('mount','-o','loop,ro',image,native)
    try:
     labelpath=next(p for p in [P(native+'/lsm-review/label'),P(native+'/cap-review/label')] if p.exists())
     assert os.getxattr(labelpath,'security.selinux')==b'system_u:object_r:foreign_build_label_t:s0\\0'
     if P(native+'/os/usr/bin/bash').exists():assert os.getxattr(native+'/os/usr/bin/bash','security.selinux')==b'system_u:object_r:foreign_build_label_t:s0\\0'
     if P(native+'/lsm-review/label-symlink').is_symlink():assert os.getxattr(native+'/lsm-review/label-symlink','security.selinux',follow_symlinks=False)==b'link-label'
     emit('NATIVE_EXT4_LABEL_PERSISTENCE_PASS')
    finally:call('umount',native)""")
(root/'matrix.py').write_text(source);shutil.copy2(here/'probe.py',root/'probe.py');(root/'runtimes').write_text('bwrap\n')
shutil.copy2(a.seed/'vmlinuz',out/'vmlinuz')
paths=sorted(root.rglob('*'));(out/'inputs.json').write_text(json.dumps({str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in paths if f.is_file() and not f.is_symlink()},indent=2)+'\n')
r=subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=b'\0'.join(str(f.relative_to(root)).encode() for f in paths)+b'\0',cwd=root,capture_output=True,check=True)
(out/'initramfs.gz').write_bytes(gzip.compress(r.stdout,compresslevel=1,mtime=0))
cmd=[v.replace(str(a.seed),str(out)) for v in json.loads((a.seed/'command.json').read_text())];(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=300,check=True)
text=(out/'results.txt').read_text();rows=[json.loads(l.split(' ',1)[1]) for l in text.splitlines() if l.startswith('RUNTIME_RESULT ')]
assert len(rows)==1 and rows[0]['status']=='PASS',rows
assert ('LSM_NO_ADMIN_METADATA_PASS' if a.lsm else 'ALL_MECHANISM_PROBES_PASS') in text and 'GUEST_EXIT 0' in text,text[-4000:]
assert ('FAILURE_POLICY_CLEARED_AFTER_QUIESCENCE' if a.cleanup_failure else 'NATIVE_EXT4_LABEL_PERSISTENCE_PASS') in text
if a.lsm:assert 'LSM_EXACT_ATTRIBUTE_PASS' in text
if a.lsm and not a.cleanup_failure:assert 'POLICY_REMOVAL_CONTROL_PASS' in text and 'OTHER_USER_NAMESPACE_CONTROL_PASS' in text
if a.cleanup_failure:assert 'LIVE_COMMAND_AND_DESCENDANT_BEFORE_FAILURE' in text
if a.kill_loader:assert 'POLICY_LOADER_KILLED_PINS_RETAINED' in text
if a.packages:assert 'FEDORA_OFFLINE_BOOTSTRAP_PASS' in text
(out/'verdict.json').write_text(json.dumps({'mechanism_probes':True,'production_qualified':False,'rows':rows},indent=2)+'\n');print(json.dumps(rows))
