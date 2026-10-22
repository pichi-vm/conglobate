#!/usr/bin/python3
"""Benign, owned-object runtime comparison. Run ONLY inside the disposable guest."""
import array,ctypes,errno,json,os,pathlib,signal,socket,subprocess,sys,time,traceback
P=pathlib.Path
libc=ctypes.CDLL(None,use_errno=True)
libc.mount.argtypes=[ctypes.c_char_p]*3+[ctypes.c_ulong,ctypes.c_char_p]
CAPS=['CAP_'+c for c in ('CHOWN','DAC_OVERRIDE','FOWNER','FSETID','SETGID','SETUID','SETPCAP','SYS_CHROOT','SYS_ADMIN','SETFCAP')]
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
 assert os.getuid() in ((0,200000) if role=='worker' else (0,)),(role,os.getuid())
 for path in ('/supervisor-secret','/context-secret','/private','/work','/run/systemd/private'):
  assert not P(path).exists(),path
 st=P('/proc/self/status').read_text()
 assert 'NoNewPrivs:\t1' in st,st
 if role=='worker':assert 'CapEff:\t0000000000000000' in st,st
 else:assert 'CapEff:\t00000000802401db' in st,st
 routes=P('/proc/net/route').read_text()
 assert all(row.split()[0]=='lo' for row in routes.splitlines()[1:] if row.strip()),routes
 emit('PRIVATE_ROUTES',routes)
 fds={}
 for p in P('/proc/self/fd').iterdir():
  try:fds[p.name]=os.readlink(p)
  except FileNotFoundError:pass
 allowed={0,1,2,3} if role=='worker' else {0,1,2}
 assert {int(k) for k,v in fds.items() if v!='/usr/lib64/libffi.so.8'}==allowed,fds
 filter_probe()
 emit('ROLE_READY',{'role':role,'namespaces':ns(),'fds':fds,'uid_map':P('/proc/self/uid_map').read_text(),'caps':next(x for x in st.splitlines() if x.startswith('CapEff:'))})
 if role=='worker':os.execv('/usr/bin/fuse-worker',['fuse-worker','-f','-o','kernel,acl,allow_other,nosuid,nodev','/storage/image','/dev/fd/3'])
 assert not P('/storage/image').exists()
 buf=ctypes.create_string_buffer(256);assert libc.statfs(b'/image',buf)==0;magic=ctypes.c_long.from_buffer(buf).value;assert magic==0x65735546,hex(magic)
 emit('FUSE_FILESYSTEM_VERIFIED',magic)
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
  s.sendmsg([b'tree'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[tree]))]);os.close(tree)
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
 mounts=[('/usr','/usr',True),('/matrix.py','/matrix.py',True),('/checks.py','/checks.py',True),('/expect-placement-fixed','/expect-placement-fixed',True)]
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

if P('/runtimes').read_text().strip()=='legacy':
 os.execv('/usr/bin/python3',['python3','/topology.py'])
P('/private').mkdir(exist_ok=True);os.chmod('/private',0o711)
P('/supervisor-secret').write_text('unchanged');P('/context-secret').write_text('unchanged')
os.chmod('/dev/fuse',0o666)
outer=ns();results=[]
emit('GUEST_KERNEL',list(os.uname()))
P('/run/systemd/journal').mkdir(parents=True,exist_ok=True)
journal=socket.socket(socket.AF_UNIX,socket.SOCK_DGRAM);journal.bind('/run/systemd/journal/socket');journal.setblocking(False)
call('systemctl','start','dbus.service')

def roles(log):
 records=[]
 for l in log.read_text().splitlines():
  if l.startswith('ROLE_READY '):records.append(json.loads(l[len('ROLE_READY '):]))
 return records

def live_in(records):
 ids={r['namespaces']['pid'] for r in records};found=[]
 for p in P('/proc').iterdir():
  if not p.name.isdigit():continue
  try:
   if os.readlink(p/'ns/pid') in ids and (p/'stat').read_text().split(') ',1)[1][0]!='Z':found.append(int(p.name))
  except (FileNotFoundError,ProcessLookupError):pass
 return found

def wait_marker(p,log,marker,seconds=8):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if marker in log.read_text():return
  if p.poll() is not None:raise RuntimeError(f'early exit {p.returncode}: '+log.read_text()[-1800:])
  time.sleep(.02)
 raise TimeoutError(marker+': '+log.read_text()[-1600:])

emit('MANAGER_PATHS',[str(p) for p in P('/run/systemd').glob('*')])
runtimes=P('/runtimes').read_text().split()
for runtime in runtimes:
 for case in ('normal','exit42','descendant','launcher-death','worker-death','timeout','helper-death'):
  ident='rv-'+runtime+'-'+case;image='/private/'+ident+'.ext4';view='/work/'+ident
  P(view).mkdir();os.chown(view,100000,100000)
  call('truncate','-s','64M',image);call('mkfs.ext4','-q','-F',image);os.chown(image,200000,200000)
  a,b=socket.socketpair();a.settimeout(10)
  h=subprocess.Popen(['unshare','--user','--mount','--map-users=0:100000:65536','--map-groups=0:100000:65536','--setuid=0','--setgid=0','python3','/matrix.py','helper',str(b.fileno()),view],pass_fds=[b.fileno()]);b.close()
  wp=cp=None;files=[];logs=[];extra=None;fd=None
  result={'runtime':runtime,'case':case,'status':'FAIL'}
  try:
   data,anc,_,_=a.recvmsg(100,socket.CMSG_SPACE(4));owner=json.loads(data)['pid'];fd=array.array('i',anc[0][2])[0]
   exported=view+'-export';P(exported).mkdir()
   if runtime=='systemd':
    extra=subprocess.Popen(['unshare','--user','--map-users=0:200000:65536','--map-groups=0:200000:65536','--setuid=0','--setgid=0','sleep','120'])
    time.sleep(.03)
   for role in ('worker','command'):
    lp=P('/work')/(ident+'-'+role+'.log');log=open(lp,'w');files.append(log);logs.append(lp)
    ownerarg=extra.pid if role=='worker' and extra else owner
    cmd=['python3','/matrix.py','launch',runtime,role,case,image,exported,str(ownerarg),ident+'-'+role,str(fd if role=='worker' else -1)]
    p=subprocess.Popen(cmd,pass_fds=[fd] if role=='worker' else [],stdin=subprocess.DEVNULL,stdout=log,stderr=log)
    if role=='worker':
     wp=p;os.close(fd);fd=None;wait_marker(wp,lp,'ROLE_READY')
     a.send(b'export');data,anc,_,_=a.recvmsg(32,socket.CMSG_SPACE(4));assert data==b'tree';tree=array.array('i',anc[0][2])[0]
     assert libc.syscall(429,tree,b'',-100,exported.encode(),4)==0,ctypes.get_errno();os.close(tree)
    else:cp=p
   if case in ('launcher-death','worker-death','timeout','helper-death'):
    wait_marker(cp,logs[1],'COMMAND_WAITING')
    if case=='launcher-death':cp.kill()
    if case=='timeout':
     for pid in live_in(roles(logs[1])):os.kill(pid,signal.SIGKILL)
    if case=='worker-death':
     # Kill the actual owned sandbox init/payload, not just an OCI client.
     for pid in live_in(roles(logs[0])):os.kill(pid,signal.SIGKILL)
    if case=='helper-death':os.kill(owner,signal.SIGKILL)
   try:rc=cp.wait(timeout=3)
   except subprocess.TimeoutExpired:rc='still-running'
   deadline=time.monotonic()+.5
   while rc!='still-running' and live_in(roles(logs[1])) and time.monotonic()<deadline:time.sleep(.01)
   result['command_rc']=rc
   records=roles(logs[0])+roles(logs[1]);assert len(records)==2,records
   for key in NAMES:assert len({outer[key],*[r['namespaces'][key] for r in records]})==3,(key,records)
   result.update(command_rc=rc,metadata='METADATA_PASS' in logs[1].read_text(),live_command_after_exit=live_in(roles(logs[1])))
   if case in ('normal','exit42','descendant'):
    assert rc==(42 if case=='exit42' else 0),rc
    assert not result['live_command_after_exit'],result
   # Capture observations before the outer harness performs owned-object cleanup.
   result['status']='PASS' if case in ('normal','exit42','descendant') else 'OBSERVED'
   if case in ('normal','exit42','descendant'):
    call('umount',exported);a.send(b'unmount');assert a.recv(32)==b'done';assert h.wait(timeout=3)==0
    result['worker_rc']=wp.wait(timeout=3);assert result['worker_rc']==0,result
    result['fsck']=call('e2fsck','-fn',image).splitlines()[-1]
   if case=='worker-death':assert 'WORKER_IO_FAILED' in logs[1].read_text(),logs[1].read_text()
  except Exception as e:result['status']='FAIL';result['error']=str(e);emit('CASE_EXCEPTION',traceback.format_exc())
  finally:
   for lp in logs:
    for pid in live_in(roles(lp)):
     try:os.kill(pid,signal.SIGKILL)
     except ProcessLookupError:pass
   if fd is not None:os.close(fd)
   subprocess.run(['umount',view+'-export'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   if h.poll() is None:
    try:a.send(b'unmount');a.recv(32)
    except Exception as e:result['unmount_error']=str(e)
   for p in (cp,wp,h,extra):
    if p is not None:
     try:p.wait(timeout=2)
     except subprocess.TimeoutExpired:p.kill();p.wait(timeout=2)
   for runtimeid in (ident+'-command',ident+'-worker'):
    if runtime in ('crun','runc','youki','libcontainer'):
     command=['youki' if runtime=='libcontainer' else runtime,'--root','/run/'+runtime]
     if runtime=='crun':command+=['--cgroup-manager=disabled']
     subprocess.run(command+['delete','--force',runtimeid],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    if runtime=='systemd':subprocess.run(['systemctl','stop',runtimeid],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   while True:
    try:emit('JOURNAL_RECORD',journal.recv(65536).decode(errors='replace'))
    except BlockingIOError:break
   for f in files:f.close()
   for lp in logs:emit('ACTOR_LOG',{'runtime':runtime,'case':case,'file':str(lp),'text':lp.read_text()})
   a.close()
   result['connections_remaining']=len(list(P('/sys/fs/fuse/connections').iterdir()))
   result['live_after_cleanup']=live_in([r for lp in logs for r in roles(lp)])
   if result['connections_remaining'] or result['live_after_cleanup']:result['status']='FAIL';result['cleanup_failed']=True
   emit('RUNTIME_RESULT',result);results.append(result)
  if result['status']=='FAIL':break
assert P('/supervisor-secret').read_text()=='unchanged';assert P('/context-secret').read_text()=='unchanged'
emit('RUNTIME_MATRIX_COMPLETE',results)
