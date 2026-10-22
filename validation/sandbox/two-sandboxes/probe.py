#!/usr/bin/python3
"""Two sibling sandboxes; all probes target owned fixtures in a disposable VM."""
import os,sys,socket,array,subprocess,time,pathlib,ctypes,errno,json,signal
P=pathlib.Path
libc=ctypes.CDLL(None,use_errno=True)
libc.mount.argtypes=[ctypes.c_char_p]*3+[ctypes.c_ulong,ctypes.c_char_p]
def emit(s):print(s,flush=True)
def run(*c):subprocess.run(c,check=True)
def ns():return {n:os.readlink('/proc/self/ns/'+n) for n in ('user','mnt','pid','net','ipc','uts')}
def isolated(role,port):
 assert os.getuid()==(200000 if role=='worker' else 0)
 assert P('/proc/1').exists()
 for hidden in ('/supervisor-secret','/context-secret','/private','/work','/peer-private'):
  assert not P(hidden).exists(),hidden
 # Only fixture-owned endpoints. Each has a positive supervisor-side control.
 for family,address in [(socket.AF_INET,('127.0.0.1',port)),(socket.AF_UNIX,'\0topology-supervisor')]:
  with socket.socket(family,socket.SOCK_STREAM) as s:
   s.settimeout(.5)
   try:s.connect(address)
   except OSError as e:assert e.errno in (errno.ECONNREFUSED,errno.ENOENT,errno.ENETUNREACH)
   else:raise AssertionError('supervisor endpoint exposed')
 fds=[]
 for p in P('/proc/self/fd').iterdir():
  try:t=os.readlink(p)
  except FileNotFoundError:continue
  emit("FD_OBSERVATION "+role+" "+p.name+" "+t)
  if t=='/usr/lib64/libffi.so.8':
   import fcntl
   assert fcntl.fcntl(int(p.name),fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
   continue
  fds.append(int(p.name))
  assert 'supervisor-secret' not in t and 'context-secret' not in t
 assert set(fds)==({0,1,2,3} if role=='worker' else {0,1,2}),fds
 # The fresh PID namespace contains only sandbox processes.
 for p in P('/proc').iterdir():
  if p.name.isdigit():
   try:st=(p/'status').read_text()
   except FileNotFoundError:continue
   uid=int(next(l.split()[1] for l in st.splitlines() if l.startswith('Uid:')))
   assert uid==(200000 if role=='worker' else 0),(role,uid)
 emit('ISOLATION_PASS '+role+' '+json.dumps(ns(),sort_keys=True))

def common():
 return ['bwrap','--die-with-parent','--new-session','--unshare-pid','--unshare-net','--unshare-ipc','--unshare-uts',
 '--ro-bind','/usr','/usr','--symlink','usr/bin','/bin','--symlink','usr/sbin','/sbin',
 '--symlink','usr/lib64','/lib64','--symlink','usr/lib','/lib','--proc','/proc','--dev','/dev',
 '--tmpfs','/tmp','--ro-bind','/topology.py','/topology.py','--clearenv','--setenv','PATH','/usr/bin:/usr/sbin',
 '--setenv','HOME','/tmp','--chdir','/']

if len(sys.argv)>1:
 mode=sys.argv[1]
 if mode=='worker-launch':
  fd=int(sys.argv[2]);image=sys.argv[3];port=sys.argv[4]
  os.dup2(fd,0,inheritable=True)
  if fd!=0:os.close(fd)
  os.setgroups([]);os.setgid(200000);os.setuid(200000)
  c=common()+['--unshare-user','--cap-drop','ALL','--bind',image,'/storage/image',
    'python3','/topology.py','worker',port]
  os.execvp(c[0],c)
 if mode=='worker':
  os.dup2(0,3,inheritable=True)
  null=os.open('/dev/null',os.O_RDONLY);os.dup2(null,0);os.close(null)
  isolated('worker',int(sys.argv[2]))
  st=P('/proc/self/status').read_text();assert 'CapEff:\t0000000000000000' in st
  assert 'NoNewPrivs:\t1' in st
  os.execv('/usr/bin/fuse-worker',['fuse-worker','-f','-o','kernel,acl,allow_other,nosuid,nodev','/storage/image','/dev/fd/3'])
 if mode=='command':
  isolated('command',int(sys.argv[2]));stage=sys.argv[3]
  assert not P('/storage').exists()
  import checks
  checks.metadata('/image',0,'same-userns-control')
  P('/image/finished').write_text(stage)
  if stage in ('command-death','helper-death','worker-death'):
   emit('COMMAND_WAITING')
   if stage=='worker-death':
    for _ in range(500):
     try:P('/image/finished').read_bytes()
     except OSError as e:
      assert e.errno in (errno.EIO,errno.ENOTCONN,errno.ECONNABORTED);emit('WORKER_DEATH_IO_REJECTED '+str(e.errno));break
     time.sleep(.01)
    else:raise AssertionError('worker death not observed')
   else:time.sleep(30)
  sys.exit(42 if stage=='command-failure' else 0)
 if mode=='helper':
  s=socket.socket(fileno=int(sys.argv[2]));stage=sys.argv[3];port=sys.argv[4]
  fd=os.open('/dev/fuse',os.O_RDWR)
  opts=f'fd={fd},rootmode=40000,user_id=0,group_id=0,allow_other,default_permissions'.encode()
  assert libc.mount(b'topology',b'/work/view',b'fuse',6,opts)==0,ctypes.get_errno()
  s.sendmsg([b'mounted'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[fd]))]);os.close(fd)
  assert s.recv(100)==b'go'
  c=common()+['--bind','/work/view','/image','--ro-bind','/checks.py','/checks.py',
    '--ro-bind','/expect-placement-fixed','/expect-placement-fixed']
  for cap in ('CHOWN','DAC_OVERRIDE','FOWNER','FSETID','SETGID','SETUID','SETPCAP','SYS_CHROOT'):
   c+=['--cap-add','CAP_'+cap]
  c+=['python3','/topology.py','command',port,stage]
  child=subprocess.Popen(c,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,bufsize=1)
  for line in child.stdout:
   print(line,end='',flush=True)
   if line.strip()=='COMMAND_WAITING':
    s.send(b'command-live');assert s.recv(100)==b'continue'
    if stage=='command-death':child.kill()
  rc=child.wait(timeout=5)
  assert rc==({'command-failure':42,'command-death':-9}.get(stage,0)),rc
  run('umount','/work/view');s.send(b'done');sys.exit(0)
 raise AssertionError(mode)

emit('TOPOLOGY_BEGIN')
P('/supervisor-secret').write_text('supervisor-only');P('/context-secret').write_text('context-only')
# Readable sentinels prove mount exclusion rather than relying on file permissions.
os.chmod('/supervisor-secret',0o644);os.chmod('/context-secret',0o644)
P('/private').mkdir(mode=0o711);P('/work/view').mkdir();os.chown('/work/view',100000,100000)
os.chmod('/dev/fuse',0o666)
import fcntl,struct
with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as ctl:
 fcntl.ioctl(ctl,0x8914,struct.pack('16sH22x',b'lo',1))
tcp=socket.socket();tcp.bind(('127.0.0.1',0));tcp.listen();port=tcp.getsockname()[1]
uds=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);uds.bind('\0topology-supervisor');uds.listen()
for fam,addr in [(socket.AF_INET,('127.0.0.1',port)),(socket.AF_UNIX,'\0topology-supervisor')]:
 with socket.socket(fam,socket.SOCK_STREAM) as s:s.connect(addr)
emit('SUPERVISOR_ENDPOINT_CONTROLS_PASS '+json.dumps(ns(),sort_keys=True))
for stage in ('normal','command-failure','command-death','worker-death','helper-death'):
 image='/private/'+stage;run('truncate','-s','64M',image);run('mkfs.ext4','-q','-F',image)
 os.chown(image,200000,200000);os.chmod(image,0o600)
 a,b=socket.socketpair();a.settimeout(20)
 h=subprocess.Popen(['unshare','--user','--mount','--map-users','0:100000:65536','--map-groups','0:100000:65536','--setuid','0','--setgid','0','python3','/topology.py','helper',str(b.fileno()),stage,str(port)],pass_fds=[b.fileno()]);b.close()
 w=None
 try:
  data,anc,flags,addr=a.recvmsg(100,socket.CMSG_SPACE(4));assert data==b'mounted'
  fd=array.array('i',anc[0][2])[0]
  workerlog=open('/work/'+stage+'.worker.log','w+')
  w=subprocess.Popen(['python3','/topology.py','worker-launch',str(fd),image,str(port)],pass_fds=[fd],stdin=subprocess.DEVNULL,stdout=workerlog,stderr=workerlog)
  os.close(fd);a.send(b'go')
  if stage in ('command-death','worker-death','helper-death'):
   assert a.recv(100)==b'command-live'
   if stage=='worker-death':w.kill();assert w.wait(timeout=5)==-9
   if stage=='helper-death':h.kill()
   else:a.send(b'continue')
  if stage!='helper-death':assert a.recv(100)==b'done'
  assert h.wait(timeout=5)==(-9 if stage=='helper-death' else 0)
  assert w.wait(timeout=5)==(-9 if stage=='worker-death' else 0)
  assert not list(P('/sys/fs/fuse/connections').iterdir())
  if stage in ('helper-death','worker-death'):
   emit('FAILED_BUILD_NOT_PUBLISHED '+stage)
   emit('TOPOLOGY_CASE_PASS '+stage)
   continue
  run('e2fsck','-fn',image);run('mount','-o','loop,ro',image,'/work/view')
  assert P('/work/view/finished').read_text()==stage
  f=P('/work/view/owned');assert (f.stat().st_uid,f.stat().st_gid)==(1234,2345)
  assert os.getxattr(f,'security.selinux')==b'system_u:object_r:conglobate_foreign_t:s0\0'
  assert os.getxattr(f,'security.capability').hex()=='0100000200040000000000000000000000000000'
  assert P('/work/view/sg').stat().st_mode&0o2000
  run('umount','/work/view')
  assert P('/supervisor-secret').read_text()=='supervisor-only'
  assert P('/context-secret').read_text()=='context-only'
  emit('TOPOLOGY_CASE_PASS '+stage)
 finally:
  if h.poll() is None:h.kill();h.wait()
  if w is not None:
   if w.poll() is None:w.kill();w.wait()
   workerlog.seek(0);print(workerlog.read(),flush=True);workerlog.close()
  a.close()
assert P('/supervisor-secret').read_text()=='supervisor-only'
assert P('/context-secret').read_text()=='context-only'
ancestors={os.getpid()}
ancestor=os.getppid()
while ancestor:
 ancestors.add(ancestor)
 ancestor=int(next(l.split()[1] for l in P(f'/proc/{ancestor}/status').read_text().splitlines() if l.startswith('PPid:')))
for _ in range(100):
 live=[]
 for p in P('/proc').iterdir():
  if not p.name.isdigit() or int(p.name) in ancestors:continue
  try:
   cmd=(p/'cmdline').read_bytes();st=(p/'stat').read_text()
  except (FileNotFoundError,ProcessLookupError):continue
  if b'/topology.py' in cmd or b'fuse-worker' in cmd or b'bwrap' in cmd:live.append(int(p.name))
 if not live:break
 time.sleep(.01)
assert not live,live
emit('NO_SANDBOX_PROCESSES_REMAIN')
emit('TWO_SANDBOX_MATRIX_COMPLETE')
