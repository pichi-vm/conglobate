import os,errno,resource,signal,time
# tmpfs blocks/inodes: bounds are provisioned by the environment.
try:
 with open('/work/limited/big','wb') as f:f.write(b'x'*(2*1024*1024));f.flush()
except OSError as e:assert e.errno==errno.ENOSPC
else:raise AssertionError('tmpfs capacity not enforced')
os.unlink('/work/limited/big')
for i in range(100):
 try:open('/work/limited/'+str(i),'w').close()
 except OSError as e:assert e.errno==errno.ENOSPC;break
else:raise AssertionError('inode limit not enforced')
print('TMPFS_BLOCK_AND_INODE_EXHAUSTION_PASS',flush=True)
pid=os.fork()
if pid==0:
 resource.setrlimit(resource.RLIMIT_NOFILE,(32,32));files=[]
 try:
  for i in range(64):files.append(os.open('/dev/null',os.O_RDONLY))
 except OSError as e:os._exit(0 if e.errno==errno.EMFILE else 1)
 os._exit(1)
assert os.waitpid(pid,0)[1]==0
print('FD_EXHAUSTION_PASS',flush=True)
base='/sys/fs/cgroup'
open(base+'/cgroup.subtree_control','w').write('+pids +memory')
g=base+'/commands';os.mkdir(g)
open(g+'/pids.max','w').write('16');open(g+'/memory.max','w').write(str(64*1024*1024));open(g+'/memory.swap.max','w').write('0')
pid=os.fork()
if pid==0:
 open(g+'/cgroup.procs','w').write(str(os.getpid()))
 children=[];rejected=False
 try:
  for i in range(30):
   c=os.fork()
   if c==0:time.sleep(30);os._exit(0)
   children.append(c)
 except OSError as e:rejected=e.errno==errno.EAGAIN
 finally:
  for c in children:os.kill(c,signal.SIGKILL)
  for c in children:os.waitpid(c,0)
 os._exit(0 if rejected else 1)
assert os.waitpid(pid,0)[1]==0
print('PIDS_EXHAUSTION_PASS',flush=True)
pid=os.fork()
if pid==0:
 open(g+'/cgroup.procs','w').write(str(os.getpid()))
 data=bytearray(256*1024*1024)
 os._exit(1)
status=os.waitpid(pid,0)[1]
assert os.WIFSIGNALED(status) and os.WTERMSIG(status)==signal.SIGKILL,status
print('MEMORY_CGROUP_OOM_SUPERVISOR_SURVIVES_PASS',flush=True)
assert open(g+'/cgroup.procs').read()==''
os.rmdir(g)
print('RESOURCE_VALIDATION_PASS',flush=True)
