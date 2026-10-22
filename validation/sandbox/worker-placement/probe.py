#!/usr/bin/python3
"""Owned disposable filesystem and process probes, inside an isolated VM."""
import os,sys,socket,array,subprocess,time,pathlib,struct,errno,signal,ctypes,json
P=pathlib.Path
FIXED=P("/expect-placement-fixed").exists()
libc=ctypes.CDLL(None,use_errno=True)
libc.mount.argtypes=[ctypes.c_char_p]*3+[ctypes.c_ulong,ctypes.c_char_p]
def run(*a): return subprocess.run(a,check=True)
def emit(s): print(s,flush=True)
def deny(fn):
 try: fn()
 except OSError as e: assert e.errno in (errno.EPERM,errno.EACCES),e
 else: raise AssertionError('unexpected permission')
def child(uid,gid,groups,fn):
 p=os.fork()
 if p==0:
  try:
   os.setgroups(groups);os.setgid(gid);os.setuid(uid);fn()
  except BaseException as e: emit('CHILD_ERROR '+repr(e));os._exit(1)
  os._exit(0)
 assert os.waitpid(p,0)[1]==0

def chmod_matrix(b):
 # Each decision is also exercised against native ext4 below.
 for name,uid,gid,groups,keep in [
  ('primary',1234,3456,[],True),
  ('supplementary',1234,1234,[3456],True),
  ('many-groups',1234,1234,list(range(4000,4050))+[3456],True),
  ('nonmember',1234,1234,[3457],False)]:
  f=b/('mode-'+name);f.write_text('x');os.chown(f,1234,3456)
  child(uid,gid,groups,lambda: os.chmod(f,0o2750))
  assert bool(f.stat().st_mode&0o2000)==keep,name
  child(1235,1235,[3456],lambda: deny(lambda: os.chmod(f,0o777)))
 for name,opts,keep in [
  ('capable',['--reuid=1234','--regid=1234','--clear-groups','--inh-caps=+fsetid','--ambient-caps=+fsetid'],True),
  ('root-no-fsetid',['--bounding-set=-fsetid'],False)]:
  f=b/('mode-'+name);f.write_text('x');os.chown(f,1234,3456)
  code="import os,pathlib,sys; caps=int(next(l.split()[1] for l in pathlib.Path('/proc/self/status').read_text().splitlines() if l.startswith('CapEff:')),16); assert bool(caps & 16)==bool(int(sys.argv[2])); os.chmod(sys.argv[1],0o2750)"
  run('setpriv',*opts,'python3','-c',code,str(f),str(int(keep)))
  assert bool(f.stat().st_mode&0o2000)==keep,name
 emit('CHMOD_PERMISSION_MATRIX_PASS')

def metadata(base,worker,stage):
 b=P(base); f=b/'owned';f.write_text('content');os.chown(f,1234,2345)
 assert (f.stat().st_uid,f.stat().st_gid)==(1234,2345)
 os.setxattr(f,'security.selinux',b'system_u:object_r:conglobate_foreign_t:s0\0')
 cap=bytes.fromhex('0100000200040000000000000000000000000000')
 os.setxattr(f,'security.capability',cap);assert os.getxattr(f,'security.capability')==cap
 a=b/'acl';a.write_text('acl');a.chmod(0o600)
 run('setfacl','-m','u:1234:r,g:3456:r',str(a))
 child(1234,1234,[],lambda: a.read_bytes())
 child(1235,1235,[],lambda: deny(a.read_bytes))
 child(1235,1235,[3456],lambda: a.read_bytes())
 d=b/'inherit';d.mkdir();run('setfacl','-m','d:u:1234:r-x',str(d))
 (d/'child').write_text('inherited');assert os.getxattr(d/'child','system.posix_acl_access')
 # Worker-side group lookup is needed for chmod preserving setgid.
 sg=b/'sg';sg.write_text('group');os.chown(sg,1234,3456)
 child(1234,1234,[3456],lambda: os.chmod(sg,0o2750))
 preserved=bool(sg.stat().st_mode & 0o2000)
 emit('SUPPLEMENTARY_SETGID '+json.dumps({'preserved':preserved,'mode':oct(sg.stat().st_mode&0o7777)}))
 assert preserved == (FIXED or stage=='same-userns-control'), ('unexpected group behavior',stage,preserved)
 emit('SUPPLEMENTARY_GROUP_'+('CONTROL_PASS' if preserved else 'PLACEMENT_LIMITATION'))
 if FIXED: chmod_matrix(b)
 if stage=='same-userns-control': return
 deny(lambda: os.kill(worker,0))
 deny(lambda: os.open(f'/proc/{worker}/fd',os.O_RDONLY|os.O_DIRECTORY))
 # Only the fixture-owned worker is queried; no state is modified on denial.
 r=libc.ptrace(0x4206,worker,0,0) # PTRACE_SEIZE
 assert r==-1 and ctypes.get_errno()==errno.EPERM
 emit('METADATA_AND_WORKER_EXCLUSION_PASS')

if len(sys.argv)>1:
 mode=sys.argv[1]
 if mode=='helper':
  s=socket.socket(fileno=int(sys.argv[2]));stage=sys.argv[3]
  fd=os.open('/dev/fuse',os.O_RDWR)
  opts=f'fd={fd},rootmode=40000,user_id=0,group_id=0,allow_other,default_permissions'.encode()
  assert libc.mount(b'placement',b'/work/view',b'fuse',6,opts)==0,ctypes.get_errno()
  s.sendmsg([b'mounted'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[fd]))]);os.close(fd)
  worker=int(s.recv(100));emit('CLIENT_USERNS '+os.readlink('/proc/self/ns/user'))
  if stage=='helper-death':
   s.send(b'ready');time.sleep(30)
  elif stage=='worker-death':
   assert P('/work/view').stat();s.send(b'ready');assert s.recv(100)==b'dead'
   try: (P('/work/view')/'after-death').write_text('x')
   except OSError as e: assert e.errno in (errno.ENOTCONN,errno.EIO);emit('WORKER_DEATH_IO_REJECTED')
   else: raise AssertionError('dead worker accepted write')
  else:
   metadata('/work/view',worker,stage)
   if stage=='command-death':
    p=subprocess.Popen(['python3','-c','import time; time.sleep(30)'])
    p.kill();assert p.wait(timeout=3)==-9
    emit('COMMAND_DEATH_REAPED')
   if stage=='command-failure':
    p=subprocess.run(['python3','-c','raise SystemExit(42)']);assert p.returncode==42
    emit('COMMAND_FAILURE_OBSERVED')
  run('umount','/work/view');sys.exit(0)
 sys.exit(2)

emit('PLACEMENT_BEGIN')
P('/private').mkdir(mode=0o711);os.chown('/private',200000,200000)
P('/work/view').mkdir();os.chown('/work/view',100000,100000)
os.chmod('/dev/fuse',0o666)
if FIXED:
 run('truncate','-s','64M','/private/native');run('mkfs.ext4','-q','-F','/private/native')
 run('mount','-o','loop','/private/native','/work/view');chmod_matrix(P('/work/view'))
 run('umount','/work/view');run('e2fsck','-fn','/private/native');emit('NATIVE_CHMOD_REFERENCE_PASS')

for stage in ['same-userns-control','normal','command-failure','command-death','helper-death','worker-death']:
 image='/private/'+stage;run('truncate','-s','64M',image);run('mkfs.ext4','-q','-F',image);os.chmod(image,0o600);os.chown(image,100000 if stage=='same-userns-control' else 200000,100000 if stage=='same-userns-control' else 200000)
 a,b=socket.socketpair();a.settimeout(10)
 h=subprocess.Popen(['unshare','--user','--mount','--map-users','0:100000:65536','--map-groups','0:100000:65536','--setuid','0','--setgid','0','python3','/placement.py','helper',str(b.fileno()),stage],pass_fds=[b.fileno()]);b.close()
 w=None
 try:
  data,anc,flags,addr=a.recvmsg(100,socket.CMSG_SPACE(4));assert data==b'mounted'
  fd=array.array('i',anc[0][2])[0]
  log=open('/work/'+stage+'.log','wb')
  command=['/worker','-f','-o','kernel,acl,allow_other,nosuid,nodev',image,f'/dev/fd/{fd}']
  if stage=='same-userns-control':
   nsfd=os.open(f'/proc/{h.pid}/ns/user',os.O_RDONLY)
   def enter():
    os.setns(nsfd,0);os.setgroups([]);os.setgid(0);os.setuid(0)
   w=subprocess.Popen(command,pass_fds=[fd,nsfd],preexec_fn=enter,stdout=log,stderr=log)
   os.close(nsfd)
  else:
   w=subprocess.Popen(['setpriv','--reuid=200000','--regid=200000','--clear-groups','--bounding-set=-all','--no-new-privs']+command,pass_fds=[fd],stdout=log,stderr=log)
  os.close(fd)
  # Wait for setpriv exec and confirm actual credentials and namespace.
  for _ in range(100):
   st=P(f'/proc/{w.pid}/status').read_text()
   if ('Uid:\t100000\t100000\t100000\t100000' if stage=='same-userns-control' else 'Uid:\t200000\t200000\t200000\t200000') in st: break
   time.sleep(.01)
  if stage=='same-userns-control':
   assert os.readlink(f'/proc/{w.pid}/ns/user')==os.readlink(f'/proc/{h.pid}/ns/user')
  else:
   assert 'Uid:\t200000\t200000\t200000\t200000' in st
   assert 'CapEff:\t0000000000000000' in st
   assert os.readlink(f'/proc/{w.pid}/ns/user')==os.readlink('/proc/self/ns/user')
  assert os.readlink(f'/proc/{h.pid}/ns/user')!=os.readlink('/proc/self/ns/user')
  emit('WORKER_IDENTITY_PASS '+stage+' '+os.readlink(f'/proc/{w.pid}/ns/user'))
  a.send(str(w.pid).encode())
  if stage in ('helper-death','worker-death'):
   assert a.recv(100)==b'ready'
   if stage=='helper-death': h.kill()
   else: w.kill();w.wait(timeout=5);a.send(b'dead')
  rc=h.wait(timeout=15);assert rc==(-9 if stage=='helper-death' else 0),rc
  if stage=='helper-death':
   # Dropping the sole mount namespace unmounts the filesystem.
   emit('HELPER_DEATH_MOUNT_RELEASED')
  wr=w.wait(timeout=5);assert wr==(-9 if stage=='worker-death' else 0),wr
  assert not list(P('/sys/fs/fuse/connections').iterdir())
  if stage in ('same-userns-control','normal','command-failure','command-death'):
   run('e2fsck','-fn',image)
   run('mount','-o','loop,ro',image,'/work/view')
   f=P('/work/view/owned');assert(f.stat().st_uid,f.stat().st_gid)==(1234,2345)
   assert os.getxattr(f,'security.selinux')==b'system_u:object_r:conglobate_foreign_t:s0\0'
   assert os.getxattr(f,'security.capability').hex()=='0100000200040000000000000000000000000000'
   assert os.getxattr('/work/view/acl','system.posix_acl_access')
   assert bool(P('/work/view/sg').stat().st_mode&0o2000)==(FIXED or stage=='same-userns-control')
   run('umount','/work/view');emit('INDEPENDENT_REMOUNT_PASS '+stage)
  emit('CLEANUP_PASS '+stage)
 finally:
  if h.poll() is None: h.kill();h.wait()
  if w is not None and w.poll() is None: w.kill();w.wait()
  if w is not None:
   log.close();emit(P('/work/'+stage+'.log').read_text())
  a.close()
emit('PLACEMENT_MATRIX_COMPLETE')
