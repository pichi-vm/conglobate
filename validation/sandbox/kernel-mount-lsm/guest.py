# Appended to the previously validated command-launch definitions in matrix.py.
# Disposable VM only: all mounts and policy changes occur inside the guest.
P('/private').mkdir(exist_ok=True);os.chmod('/private',0o711)
P('/supervisor-secret').write_text('unchanged');P('/context-secret').write_text('unchanged')
image='/private/kernel.ext4';native='/work/native';view='/work/idmapped'
for path in (native,view,'/work/policy-bpf'):P(path).mkdir()
call('truncate','-s','2G',image);call('mkfs.ext4','-q','-F',image)
call('mount','-o','loop,nosuid,nodev',image,native)
h=subprocess.Popen(['unshare','--user','--map-users=0:100000:65536','--map-groups=0:100000:65536','--setuid=0','--setgid=0','python3','-u','-c','import os,time; print(os.getpid(),flush=True); time.sleep(240)'],stdout=subprocess.PIPE,text=True)
owner=int(h.stdout.readline());nsfd=os.open(f'/proc/{owner}/ns/user',os.O_RDONLY)
nsnum=int(os.readlink(f'/proc/{owner}/ns/user').split('[')[1][:-1]);sbns=int(os.readlink('/proc/self/ns/user').split('[')[1][:-1])
assert nsnum!=sbns
class Attr(ctypes.Structure):_fields_=[('attr_set',ctypes.c_ulonglong),('attr_clr',ctypes.c_ulonglong),('propagation',ctypes.c_ulonglong),('userns_fd',ctypes.c_ulonglong)]
tree=libc.syscall(428,-100,native.encode(),1|0o2000000);assert tree>=0,ctypes.get_errno()
attr=Attr(0x100000,0,0,nsfd)
assert libc.syscall(442,tree,b'',0x1000,ctypes.byref(attr),ctypes.sizeof(attr))==0,ctypes.get_errno()
assert libc.syscall(429,tree,b'',-100,view.encode(),4)==0,ctypes.get_errno();os.close(tree)
assert os.stat(native).st_uid==0 and os.stat(view).st_uid==100000
# A second, writable idmapped ext4 mount has the same superblock owner but
# must not receive the labeling exception intended for the output filesystem.
other='/work/other';othernative='/work/other-native'
for path in (other,othernative):P(path).mkdir()
call('truncate','-s','64M','/private/other.ext4');call('mkfs.ext4','-q','-F','/private/other.ext4')
call('mount','-o','loop,nosuid,nodev','/private/other.ext4',othernative)
tree=libc.syscall(428,-100,othernative.encode(),1|0o2000000);assert tree>=0
assert libc.syscall(442,tree,b'',0x1000,ctypes.byref(attr),ctypes.sizeof(attr))==0
assert libc.syscall(429,tree,b'',-100,other.encode(),4)==0;os.close(tree)
assert os.stat(other).st_dev!=os.stat(view).st_dev
print('KERNEL_IDMAP_READY',flush=True)
call('mount','-t','bpf','bpf','/work/policy-bpf')
policy=None;commands=[];logs=[]
def live():
 ids=set()
 for log in logs:
  for line in log.read_text().splitlines():
   if line.startswith('ROLE_READY '):ids.add(json.loads(line.split(' ',1)[1])['namespaces']['pid'])
 found=[]
 for p in P('/proc').iterdir():
  if not p.name.isdigit():continue
  try:
   if os.readlink(p/'ns/pid') in ids and (p/'stat').read_text().split(') ',1)[1][0]!='Z':found.append(int(p.name))
  except (FileNotFoundError,ProcessLookupError):pass
 return found

def command(case,ownerpid=owner):
 log=P('/work/'+case+'.log');logs.append(log)
 with log.open('w') as f:
  p=subprocess.Popen(['python3','/matrix.py','launch','bwrap','command',case,image,view,str(ownerpid),'kernel-'+case,'-1'],stdin=subprocess.DEVNULL,stdout=f,stderr=f)
 commands.append(p)
 if case=='cleanup-failure':
  for _ in range(800):
   if 'CLEANUP_FAILURE_READY' in log.read_text():break
   assert p.poll() is None;time.sleep(.01)
  assert 'CLEANUP_FAILURE_READY' in log.read_text()
  print('LIVE_CHILD_FAILURE_INJECTION',flush=True)
  raise RuntimeError('EXPECTED_COMMAND_CLEANUP_FAILURE')
 assert p.wait(timeout=180)==0,log.read_text()[-4000:]
 print('COMMAND_PASS '+case,flush=True)
try:
 # Both pre-install and post-removal probes use an existing label.
 P(native+'/lsm-review').mkdir();P(native+'/lsm-review/label').write_text('fixture')
 os.setxattr(native+'/lsm-review/label','security.selinux',b'system_u:object_r:foreign_build_label_t:s0\0')
 command('lsm-off')
 call('rm','-r',native+'/lsm-review')
 policy=subprocess.Popen(['/kernel-label-loader','/label-policy.bpf.o',str(nsnum),str(os.stat(native).st_dev),'/work/ready','/work/policy-bpf',str(sbns)])
 for _ in range(500):
  if P('/work/ready').exists():break
  assert policy.poll() is None;time.sleep(.01)
 assert P('/work/ready').exists()
 policy.kill();policy.wait();print('POLICY_LOADER_KILLED_PINS_RETAINED',flush=True)
 command('cleanup-failure' if P('/failure-mode').exists() else 'normal')
 foreign=subprocess.Popen(['nsenter','--user='+f'/proc/{owner}/ns/user','unshare','--user','--map-root-user','python3','-u','-c','import os,time;print(os.getpid(),flush=True);time.sleep(30)'],stdout=subprocess.PIPE,text=True)
 try:command('lsm-foreign',int(foreign.stdout.readline()))
 finally:foreign.kill();foreign.wait()
 assert not live()
 call('/kernel-label-loader','--clear','/work/policy-bpf');policy=None
 # Use a distinct launcher area for the second denial test.
 command('lsm-off-after')
 assert os.stat(native+'/lsm-review/nonroot-owned').st_uid==1234
 assert os.stat(native+'/os/usr/bin/bash').st_uid==0
 print('NATIVE_UID_PERSISTENCE_PASS',flush=True)
except RuntimeError as e:
 if str(e)!='EXPECTED_COMMAND_CLEANUP_FAILURE':raise
finally:
 # The payload cannot create namespaces. Track every command PID namespace;
 # keep policy/identity/mounts intact if quiescence cannot be established.
 if any(p.poll() is None for p in commands) and not any('ROLE_READY ' in log.read_text() for log in logs):raise RuntimeError('Unknown command scope; retain policy')
 if P('/failure-mode').exists():
  assert len(live())>=2
  print('LIVE_COMMANDS_CONFIRMED_BEFORE_FAILURE_CLEANUP',flush=True)
 deadline=time.monotonic()+5
 while live():
  for pid in live():
   try:os.kill(pid,signal.SIGKILL)
   except ProcessLookupError:pass
  assert time.monotonic()<deadline;time.sleep(.01)
 for p in commands:p.wait(timeout=2)
 print('COMMANDS_QUIESCENT',flush=True)
 if policy is not None:
  if policy.poll() is None:policy.kill();policy.wait()
  call('/kernel-label-loader','--clear','/work/policy-bpf')
 call('umount','/work/policy-bpf');call('umount',other);call('umount',othernative);call('umount',view);call('umount',native)
 h.kill();h.wait();os.close(nsfd)
 for log in logs:print('ACTOR_LOG '+log.read_text(),flush=True)
print(call('e2fsck','-fn',image),flush=True)
call('mount','-o','loop,ro,nosuid,nodev',image,native)
try:
 assert os.getxattr(native+'/lsm-review/label','security.selinux')==b'system_u:object_r:foreign_build_label_t:s0\0'
 if not P('/failure-mode').exists():assert os.getxattr(native+'/os/usr/bin/bash','security.selinux')==b'system_u:object_r:foreign_build_label_t:s0\0'
 assert os.stat(native+'/lsm-review/nonroot-owned').st_uid==1234
 assert (os.stat(native+'/owned').st_uid,os.stat(native+'/owned').st_gid)==(1234,2345)
 assert os.getxattr(native+'/owned','security.capability')==bytes.fromhex('0100000200040000000000000000000000000000')
 assert os.getxattr(native+'/acl','system.posix_acl_access')
 assert os.getxattr(native+'/inherit/child','system.posix_acl_access')
 assert os.stat(native+'/sg').st_mode&0o2000
 print('NATIVE_METADATA_PERSISTENCE_PASS',flush=True)
 print('KERNEL_REMOUNT_PERSISTENCE_PASS',flush=True)
finally:call('umount',native)
assert P('/supervisor-secret').read_text()=='unchanged' and P('/context-secret').read_text()=='unchanged'
assert not live()
assert not list(P('/sys/fs/fuse/connections').iterdir())
print('KERNEL_MOUNT_LSM_PASS',flush=True)
