import os, errno, mmap, fcntl, subprocess
p='/semantics'
os.mkdir(p)
print('ANCESTOR_MODES',oct(os.stat('/').st_mode),oct(os.stat(p).st_mode),flush=True)
os.chmod('/',0o755);os.chmod(p,0o755)
# Kernel ACL enforcement, including explicit denial for an unrelated identity.
f=p+'/acl';open(f,'w').write('secret');os.chmod(f,0o600)
subprocess.run(['setfacl','-m','u:1234:r',f],check=True)
for uid,allowed in [(1234,True),(1235,False)]:
 pid=os.fork()
 if pid==0:
  os.setgroups([]);os.setgid(uid);os.setuid(uid)
  try: open(f).read(); success=True
  except PermissionError as e: print('ACL_DENIAL',uid,repr(e),flush=True); success=False
  print('ACL_RESULT',uid,success,allowed,flush=True);os._exit(0 if success==allowed else 1)
 status=os.waitpid(pid,0)[1];print('CHILD_STATUS',status,flush=True)
print('ACL_ENFORCEMENT_PROBE_END',flush=True)
subprocess.run(['setfacl','-m','d:u:1234:rwx',p],check=True)
g=p+'/inherited';open(g,'w').close()
print('INHERITED_ACL',subprocess.check_output(['getfacl','-n',g],text=True),flush=True)
print('ACL_INHERITANCE_PROBE_END',flush=True)
try: os.chown(g,65536,65536)
except OSError as e: assert e.errno==errno.EINVAL
else: raise AssertionError('unmapped ownership accepted')
os.chown(g,65535,65535)
assert os.stat(g).st_uid==65535
print('ID_RANGE_BOUNDARY_PASS',flush=True)
f=p+'/sparse'
with open(f,'w+b') as out:
 out.truncate(16*1024*1024)
 out.seek(8*1024*1024);out.write(b'hello');out.flush()
 os.fsync(out.fileno())
 with mmap.mmap(out.fileno(),0) as m:
  assert m[8*1024*1024:8*1024*1024+5]==b'hello'
  m[0:4]=b'test';m.flush()
 fcntl.flock(out,fcntl.LOCK_EX|fcntl.LOCK_NB)
 pid=os.fork()
 if pid==0:
  other=open(f,'rb')
  try: fcntl.flock(other,fcntl.LOCK_EX|fcntl.LOCK_NB)
  except BlockingIOError: os._exit(0)
  os._exit(1)
 assert os.waitpid(pid,0)[1]==0
assert os.stat(f).st_blocks*512<os.stat(f).st_size
os.link(f,p+'/hard');os.rename(f,p+'/renamed')
assert os.stat(p+'/hard').st_ino==os.stat(p+'/renamed').st_ino
os.symlink('/semantics/renamed',p+'/link')
assert open(p+'/link','rb').read(4)==b'test'
print('SPARSE_MMAP_FSYNC_LOCK_LINK_RENAME_PASS',flush=True)
