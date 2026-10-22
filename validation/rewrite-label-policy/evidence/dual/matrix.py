#!/usr/bin/python3
"""Benign, owned-object runtime comparison. Run ONLY inside the disposable guest."""
import array,ctypes,errno,json,os,pathlib,signal,socket,subprocess,sys,time,traceback
P=pathlib.Path
libc=ctypes.CDLL(None,use_errno=True)
libc.mount.argtypes=[ctypes.c_char_p]*3+[ctypes.c_ulong,ctypes.c_char_p]
CAPS=['CAP_'+c for c in ('CHOWN','DAC_OVERRIDE','FOWNER','FSETID','SETGID','SETUID','SETPCAP','SYS_CHROOT','SETFCAP')]
NAMES=('user','mnt','pid','net','ipc','uts')
def emit(tag,data=None):print(tag+(' '+json.dumps(data,sort_keys=True) if data is not None else ''),flush=True)
def ns():return {n:os.readlink('/proc/self/ns/'+n) for n in NAMES}
def call(*args):return subprocess.run(args,check=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True).stdout

def filter_probe():
 sc=ctypes.CDLL('libseccomp.so.2',use_errno=True)
 sc.seccomp_init.argtypes=[ctypes.c_uint];sc.seccomp_init.restype=ctypes.c_void_p
 sc.seccomp_syscall_resolve_name.argtypes=[ctypes.c_char_p]
 sc.seccomp_rule_add.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_uint]
 sc.seccomp_load.argtypes=[ctypes.c_void_p];sc.seccomp_release.argtypes=[ctypes.c_void_p]
 ctx=sc.seccomp_init(0x7fff0000);assert ctx
 for name in ('mount','umount2','pivot_root','setns','unshare','ptrace','process_vm_readv','process_vm_writev','open_by_handle_at','bpf','keyctl','add_key','request_key','reboot','kexec_load','init_module','finit_module','delete_module','swapon','swapoff'):
  nr=sc.seccomp_syscall_resolve_name(name.encode());assert nr>=0,name
  assert sc.seccomp_rule_add(ctx,0x50000|errno.EPERM,nr,0)==0,name
 assert sc.seccomp_load(ctx)==0;sc.seccomp_release(ctx)
 ctypes.set_errno(0);assert libc.unshare(0)==-1 and ctypes.get_errno()==errno.EPERM
 emit('FILTER_PROBE_PASS')

def payload(role,case):
 if role=='worker-stdin':
  os.dup2(0,3,inheritable=True);f=os.open('/dev/null',os.O_RDONLY);os.dup2(f,0);os.close(f);role='worker'
 if role=='command':
  import probe;probe.clamp_bounding(False)
 assert os.getuid() in ((0,200000) if role=='worker' else (0,)),(role,os.getuid())
 for path in ('/supervisor-secret','/context-secret','/private','/work','/run/systemd/private'):
  assert not P(path).exists(),path
 st=P('/proc/self/status').read_text()
 assert 'NoNewPrivs:\t1' in st,st
 if role=='worker':assert 'CapEff:\t0000000000000000' in st,st
 else:assert 'CapEff:\t00000000800401db' in st,st
 routes=P('/proc/net/route').read_text()
 assert all(row.split()[0]=='lo' for row in routes.splitlines()[1:] if row.strip()),routes
 emit('PRIVATE_ROUTES',routes)
 fds={}
 for p in P('/proc/self/fd').iterdir():
  try:fds[p.name]=os.readlink(p)
  except FileNotFoundError:pass
 allowed={0,1,2,3} if role=='worker' else {0,1,2}
 assert {int(k) for k,v in fds.items() if v!='/usr/lib64/libffi.so.8'}==allowed,fds
 if role=='worker':filter_probe()
 emit('ROLE_READY',{'role':role,'namespaces':ns(),'fds':fds,'uid_map':P('/proc/self/uid_map').read_text(),'caps':next(x for x in st.splitlines() if x.startswith('CapEff:'))})
 if role=='worker':os.execv('/usr/bin/fuse-worker',['fuse-worker','-f','-o','kernel,acl,allow_other,nosuid,nodev','/storage/image','/dev/fd/3'])
 assert not P('/storage/image').exists()
 buf=ctypes.create_string_buffer(256);assert libc.statfs(b'/image',buf)==0;magic=ctypes.c_long.from_buffer(buf).value;assert magic==0xef53,hex(magic)
 emit('KERNEL_EXT4_VERIFIED',magic)
 import probe;probe.run_lsm(case);emit('METADATA_PASS');return
 # A child with SYS_ADMIN removed must fail the same foreign-label write.
 control="import os,errno; p='/image/no-admin'; open(p,'w').close();\ntry: os.setxattr(p,'security.selinux',b'system_u:object_r:conglobate_foreign_t:s0\\0')\nexcept OSError as e: assert e.errno==errno.EPERM\nelse: raise AssertionError('label write unexpectedly allowed')"
 call('setpriv','--bounding-set=-sys_admin','--inh-caps=-sys_admin','--ambient-caps=-sys_admin','python3','-c',control)
 emit('NO_SYS_ADMIN_LABEL_DENIED')
 import checks
 checks.metadata('/image',0,'same-userns-control')
 P('/image/finished').write_text(case)
 emit('METADATA_PASS')
 if case in ('launcher-death','worker-death','timeout','helper-death'):
  emit('COMMAND_WAITING')
  if case=='worker-death':
   for _ in range(300):
    try:P('/image/finished').read_bytes()
    except OSError as e:
     assert e.errno in (errno.EIO,errno.ENOTCONN,errno.ECONNABORTED);emit('WORKER_IO_FAILED',e.errno);return
    time.sleep(.01)
  time.sleep(20)
 if case=='descendant':
  if os.fork()==0:time.sleep(20);os._exit(0)
 sys.exit(42 if case=='exit42' else 0)

def helper(fd,view):
 s=socket.socket(fileno=fd);f=os.open('/dev/fuse',os.O_RDWR)
 opts=f'fd={f},rootmode=40000,user_id=0,group_id=0,allow_other,default_permissions'.encode()
 assert libc.mount(b'runtime-test',view.encode(),b'fuse',6,opts)==0,ctypes.get_errno()
 emit('HELPER_NS',{'pid':os.getpid(),'ns':ns(),'status':P('/proc/self/status').read_text(),'cmdline':P('/proc/cmdline').read_text()})
 s.sendmsg([json.dumps({'pid':os.getpid()}).encode()],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[f]))]);os.close(f)
 message=s.recv(32)
 if message==b'export':
  tree=libc.syscall(428,-100,view.encode(),1|0o2000000);assert tree>=0,ctypes.get_errno()
  s.sendmsg([json.dumps({'dev':os.stat(view).st_dev}).encode()],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[tree]))]);os.close(tree)
  message=s.recv(32)
 assert message==b'unmount';call('umount',view);s.send(b'done')

def layout(root):
 for path in ('usr','etc','proc','sys','dev','tmp','run','image','storage'): (root/path).mkdir(parents=True,exist_ok=True)
 for name,target in [('bin','usr/bin'),('sbin','usr/sbin'),('lib64','usr/lib64'),('lib','usr/lib')]:
  if not (root/name).exists():(root/name).symlink_to(target)
 (root/'etc/os-release').write_text('ID=conglobate-test\nNAME=Con globate\n')
 (root/'etc/passwd').write_text('root:x:0:0:root:/:/bin/sh\n')
 (root/'etc/group').write_text('root:x:0:\n')
 for name in ('matrix.py','checks.py','expect-placement-fixed'):(root/name).touch()

def launch(runtime,role,case,image,view,owner,ident,fd=-1):
 area=P('/private')/(ident+'-area');area.mkdir();os.chown(area,200000 if role=='worker' else 100000,200000 if role=='worker' else 100000)
 root=area/'root';layout(root)
 for entry in [root,*root.rglob('*')]:os.chown(entry,200000 if role=='worker' else 100000,200000 if role=='worker' else 100000,follow_symlinks=False)
 mounts=[('/work/other','/other',False),('/rpms','/rpms',True),('/pkg-etc','/etc',True),('/probe.py','/probe.py',True),('/usr','/usr',True),('/matrix.py','/matrix.py',True),('/checks.py','/checks.py',True),('/expect-placement-fixed','/expect-placement-fixed',True)]
 mounts += [(image,'/storage/image',False)] if role=='worker' else [(view,'/image',False)]
 caps=[] if role=='worker' else CAPS
 dev=P('/private')/(ident+'-null');os.mknod(dev,0o20666,os.makedev(1,3));os.chown(dev,200000 if role=='worker' else 100000,200000 if role=='worker' else 100000)
 cmd=['/usr/bin/python3','/matrix.py','payload',role,case]
 prefix=[]
 # For runtimes without a direct join interface, enter the trusted mount owner's
 # user namespace before launching. This is measured adapter work, not a feature.
 if role=='command' and runtime in ('bwrap','nsjail','nspawn','unshare'):
  prefix=['nsenter','--user='+f'/proc/{owner}/ns/user']
 if runtime=='bwrap':
  c=prefix+['bwrap','--die-with-parent','--new-session','--unshare-pid','--unshare-net','--unshare-ipc','--unshare-uts']
  if role=='worker':
   os.setgroups([]);os.setgid(200000);os.setuid(200000)
   c+=['--unshare-user']
   os.dup2(3,0,inheritable=True);os.close(3);cmd[-2]='worker-stdin'
  else:pass
  c+=['--cap-drop','ALL']
  for cap in caps:c+=['--cap-add',cap]
  c+=['--proc','/proc','--dev','/dev','--tmpfs','/tmp','--tmpfs','/run','--dir','/sys','--clearenv','--setenv','PATH','/usr/bin:/usr/sbin']
  for src,dst,ro in mounts:c+=['--ro-bind' if ro else '--bind',src,dst]
  for n,t in [('bin','usr/bin'),('sbin','usr/sbin'),('lib64','usr/lib64'),('lib','usr/lib')]:c+=['--symlink',t,'/'+n]
  c+=cmd
 elif runtime in ('crun','runc','youki','libcontainer'):
  nslist=[{'type':x} for x in ('mount','pid','network','ipc','uts')]
  nslist.append({'type':'user',**({'path':f'/proc/{owner}/ns/user'} if role=='command' else {})})
  linux={'namespaces':nslist,'cgroupsPath':'/runtime-tests/'+ident}
  if role=='worker':
   linux.update(uidMappings=[{'containerID':0,'hostID':200000,'size':65536}],gidMappings=[{'containerID':0,'hostID':200000,'size':65536}])
  linux['devices']=[]
  ms=[{'destination':'/proc','type':'proc','source':'proc','options':['nosuid','nodev','noexec']},{'destination':'/dev','type':'tmpfs','source':'tmpfs','options':['nosuid','mode=755']},{'destination':'/sys','type':'tmpfs','source':'tmpfs','options':['ro','nosuid','nodev','noexec']}]
  mounts.append((str(dev),'/dev/null',False))
  for src,dst,ro in mounts:ms.append({'destination':dst,'type':'bind','source':src,'options':['rbind','ro' if ro else 'rw','rprivate']})
  spec={'ociVersion':'1.0.2','root':{'path':str(root),'readonly':False},'hostname':'fixture','process':{'terminal':False,'user':{'uid':0,'gid':0},'args':cmd,'env':['PATH=/usr/bin:/usr/sbin','HOME=/'],'cwd':'/','noNewPrivileges':True,'capabilities':{k:caps for k in ('bounding','effective','permitted','inheritable','ambient')}},'mounts':ms,'linux':linux}
  bundle=P('/private')/(ident+'-bundle');bundle.mkdir();(bundle/'config.json').write_text(json.dumps(spec))
  c=[runtime,'--root','/run/'+runtime]
  if runtime=='crun':c+=['--cgroup-manager=disabled']
  c+=['run','--bundle',str(bundle)]
  if role=='worker':c+=['--preserve-fds','1']
  c+=[ident]
  if runtime=='libcontainer':c=['/usr/bin/libcontainer-probe',str(bundle),ident,str(1 if role=='worker' else 0)]
 elif runtime=='nsjail':
  c=prefix+['nsjail','--mode','o','--chroot',str(root),'--user','0:200000:65536' if role=='worker' else '0','--group','0:200000:65536' if role=='worker' else '0','--time_limit','0','--disable_rlimits','--disable_clone_newcgroup']
  if role=='command':c+=['--disable_clone_newuser']
  else:c+=['--pass_fd','3']
  for cap in caps:c+=['--cap',cap]
  for src,dst,ro in mounts:c+=['-R' if ro else '-B',src+':'+dst]
  c+=['-B',str(dev)+':/dev/null','--env','PATH=/usr/bin:/usr/sbin','--']+cmd
 elif runtime=='nspawn':
  if role=='command':prefix=['unshare','--mount','bash','-eu','-c','mount -t tmpfs -o uid=100000,gid=100000 tmpfs /run; exec "$@"','nspawn-adapter']+prefix+['unshare','--mount','--pid','--fork','--kill-child']
  c=prefix+['systemd-nspawn','--quiet','--machine='+ident,'--directory='+str(root),'--register=no','--keep-unit','--private-network','--as-pid2','--console=pipe','--resolv-conf=off','--link-journal=no','--timezone=off','--settings=no','--no-new-privileges=yes','--drop-capability=all']
  c+=['--private-users=200000:65536','--private-users-ownership=off'] if role=='worker' else ['--private-users=no']
  if caps:c+=['--capability='+','.join(caps)]
  for src,dst,ro in mounts:c+=['--bind-ro='+src+':'+dst if ro else '--bind='+src+':'+dst]
  if role=='worker':os.dup2(3,0,inheritable=True);os.close(3);cmd[-2]='worker-stdin'
  c+=cmd
 elif runtime=='systemd':
  c=['systemd-run','--quiet','--pipe','--unit='+ident,'--service-type=exec']
  props={'DefaultDependencies':'no','RootDirectory':str(root),'PrivatePIDs':'yes','PrivateNetwork':'yes','PrivateIPC':'yes','ProtectHostname':'yes','PrivateMounts':'yes','MountAPIVFS':'yes','NoNewPrivileges':'yes','CapabilityBoundingSet':' '.join(caps),'AmbientCapabilities':' '.join(caps),'BindReadOnlyPaths':' '.join(a+':'+b for a,b,ro in mounts if ro),'BindPaths':' '.join(a+':'+b for a,b,ro in mounts if not ro),'Environment':'PATH=/usr/bin:/usr/sbin','StandardOutput':'inherit','StandardError':'inherit','KillMode':'control-group'}
  props['User']='0'
  props['Group']=props['User']
  props['UserNamespacePath']=f'/proc/{owner}/ns/user'
  for k,v in props.items():c+=['--property='+k+'='+v]
  if role=='worker':os.dup2(3,0,inheritable=True);os.close(3);cmd[-2]='worker-stdin'
  c+=cmd
 elif runtime=='unshare':
  c=prefix+['unshare','--mount','--pid','--fork','--kill-child','--net','--ipc','--uts']
  if role=='worker':c+=['--user','--map-users=0:200000:65536','--map-groups=0:200000:65536','--setuid=0','--setgid=0']
  c+=['python3','/matrix.py','raw-setup',str(root),json.dumps(mounts),json.dumps(caps),role,case]
 else:raise ValueError(runtime)
 emit('LAUNCH',{'runtime':runtime,'role':role,'argv':c});os.execvp(c[0],c)

def raw_setup(root,mounts,caps,role,case):
 # Explicit adapter to show how much unshare leaves to its caller. No fallback
 # to host-root execution: setup must complete before the benign payload runs.
 call('mount','--bind',root,root)
 for src,dst,ro in mounts:
  target=root+dst
  if P(src).is_dir():P(target).mkdir(parents=True,exist_ok=True)
  else:P(target).parent.mkdir(parents=True,exist_ok=True);P(target).touch()
  call('mount','--bind',src,target)
  if ro:call('mount','-o','remount,bind,ro',target)
 call('mount','-t','proc','proc',root+'/proc');P(root+'/dev/null').touch();call('mount','--bind','/dev/null',root+'/dev/null')
 os.chdir(root);P('.old').mkdir();assert libc.syscall(155,b'.',b'.old')==0,ctypes.get_errno();os.chdir('/');call('umount','-l','/.old');P('/.old').rmdir()
 # setpriv controls the capability bounding set; uid0 reconstructs permitted/
 # effective only within that bound on exec. All retained caps are namespace-only.
 names=','.join('+'+x.removeprefix('CAP_').lower() for x in caps)
 c=['setpriv','--no-new-privs','--bounding-set=-all'+(','+names if names else ''),'--inh-caps=-all','--ambient-caps=-all','python3','/matrix.py','payload',role,case]
 os.execvp(c[0],c)

if len(sys.argv)>1:
 mode=sys.argv[1]
 if mode=='payload':payload(*sys.argv[2:]);sys.exit(0)
 if mode=='helper':helper(int(sys.argv[2]),sys.argv[3]);sys.exit(0)
 if mode=='launch':
  runtime,role,case,image,view,owner,ident,fd=sys.argv[2:];fd=int(fd)
  if fd>=0:
   if fd!=3:os.dup2(fd,3,inheritable=True);os.close(fd)
   else:os.set_inheritable(3,True)
  launch(runtime,role,case,image,view,int(owner),ident);sys.exit(0)
 if mode=='raw-setup':raw_setup(sys.argv[2],json.loads(sys.argv[3]),json.loads(sys.argv[4]),sys.argv[5],sys.argv[6]);sys.exit(0)


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
 policy=subprocess.Popen(['/kernel-label-loader','/label-policy.bpf.o',str(nsnum),str(os.stat(native).st_dev),'/work/ready','/work/policy-bpf',str(sbns),str(os.stat(othernative).st_dev)])
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
