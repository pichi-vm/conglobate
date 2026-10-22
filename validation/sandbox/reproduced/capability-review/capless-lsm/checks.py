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
  ('root-no-fsetid',['--bounding-set=-fsetid','--inh-caps=-fsetid','--ambient-caps=-fsetid'],False)]:
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

