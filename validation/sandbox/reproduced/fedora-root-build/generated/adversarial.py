import os
entry_fds=[]
for n in os.listdir('/proc/self/fd'):
 try: entry_fds.append((n,os.readlink('/proc/self/fd/'+n)))
 except FileNotFoundError:pass
import socket, errno, ctypes, stat, time
libc=ctypes.CDLL(None,use_errno=True)
failures=[]
def check(name,fn):
 try: fn()
 except Exception as e:
  print('SECURITY_FAILURE',name,repr(e),flush=True);failures.append(name)
 else: print('ATTACK_BLOCKED',name,flush=True)
def require_no_pid():
 pid=int(open('/daemon-pid').read())
 for name,fn in [('kill',lambda:os.kill(pid,0)),('pidfd',lambda:os.pidfd_open(pid))]:
  try: result=fn()
  except OSError as e: assert e.errno==errno.ESRCH,(name,e)
  else:
   if name=='pidfd':os.close(result)
   raise AssertionError(name+' reached worker')
 assert libc.ptrace(16,pid,None,None)==-1
 assert ctypes.get_errno()==errno.EPERM,ctypes.get_errno()
check('worker kill/pidfd/ptrace',require_no_pid)
def fds():
 for n,target in entry_fds:
  assert int(n)<=2,(n,target)
  if int(n)>0:assert stat.S_ISFIFO(os.fstat(int(n)).st_mode),(n,target)
check('inherited descriptors',fds)
def paths():
 for p in ['/work/image','/work/origin','/supervisor-secret','/.old/supervisor-secret','/context','/dev/fuse','/dev/mapper/control','/proc/1/root/../../supervisor-secret']:
  assert not os.path.exists(p),p
 os.symlink('/supervisor-secret','/escape-link')
 assert not os.path.exists('/escape-link')
check('outside paths and absolute symlink',paths)
def socket_probe():
 s=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
 try:
  s.connect('\0conglobate-trusted-canary')
 except OSError as e:assert e.errno==errno.ECONNREFUSED,e
 else:raise AssertionError('connected to trusted abstract Unix socket')
 finally:s.close()
check('abstract Unix control socket',socket_probe)
def proc_probe():
 os.mkdir('/proc-probe')
 assert libc.mount(b'proc',b'/proc-probe',b'proc',0,None)==-1
 assert ctypes.get_errno()==errno.EPERM,ctypes.get_errno()
check('new proc mount denied by policy',proc_probe)
def chroot_probe():
 pid=os.fork()
 if pid==0:
  try:
   os.mkdir('/prison');os.chdir('/');os.chroot('/prison');os.chdir('../../../../')
   assert not os.path.exists('supervisor-secret')
  except BaseException:os._exit(1)
  os._exit(0)
 assert os.waitpid(pid,0)[1]==0
check('retained cwd chroot escape stays within pivot root',chroot_probe)
# Detached grandchild must die with the step, even after setsid/double fork.
pid=os.fork()
if pid==0:
 os.setsid()
 if os.fork():os._exit(0)
 for _ in range(12000):
  with open('/detached-heartbeat','a') as f:f.write('alive\n')
  time.sleep(.03)
 os._exit(0)
os.waitpid(pid,0);time.sleep(.15)
print('ADVERSARIAL_RESULT', 'FAIL' if failures else 'PASS',flush=True)
if failures:raise SystemExit(1)
