"""Benign capability/metadata probes; only in the disposable guest fixture."""
import ctypes, errno, json, os, pathlib, socket, subprocess, sys, time
P=pathlib.Path
libc=ctypes.CDLL(None,use_errno=True)
LABEL=b'system_u:object_r:foreign_build_label_t:s0\0'
def emit(name,**kw):print('CAP_REVIEW '+json.dumps(dict(test=name,**kw),sort_keys=True),flush=True)
def must_errno(fn,expected):
 try:fn()
 except OSError as e:assert e.errno==expected,(e,expected)
 else:raise AssertionError('expected errno '+str(expected))
def syscheck(name,nr,*args,expected=errno.EPERM):
 ctypes.set_errno(0);rc=libc.syscall(nr,*args);err=ctypes.get_errno();assert rc==-1 and err==expected,(name,rc,err);emit(name,rc=rc,errno=err)
def clamp_bounding(allow_admin=True):
 allowed=0x802401db if allow_admin else 0x800401db
 for cap in range(64):
  if allowed & (1<<cap):continue
  ctypes.set_errno(0);r=libc.prctl(24,cap,0,0,0)
  assert r==0 or ctypes.get_errno()==errno.EINVAL,(cap,ctypes.get_errno())
 assert int(status()['CapBnd'],16)==allowed,status()
 emit('bounding_clamped',status=status())
def status():return {k:v.strip() for k,v in (line.split(':',1) for line in P('/proc/self/status').read_text().splitlines() if ':' in line) if k.startswith('Cap') or k in ('NoNewPrivs','Seccomp')}
class Arg(ctypes.Structure):_fields_=[('arg',ctypes.c_uint),('op',ctypes.c_uint),('a',ctypes.c_uint64),('b',ctypes.c_uint64)]
def restrict():
 sc=ctypes.CDLL('libseccomp.so.2');sc.seccomp_init.argtypes=[ctypes.c_uint];sc.seccomp_init.restype=ctypes.c_void_p
 sc.seccomp_rule_add_array.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_uint,ctypes.POINTER(Arg)]
 sc.seccomp_load.argtypes=[ctypes.c_void_p];sc.seccomp_release.argtypes=[ctypes.c_void_p];sc.seccomp_syscall_resolve_name.argtypes=[ctypes.c_char_p]
 ctx=sc.seccomp_init(0x50000|errno.EPERM);assert ctx
 def rule(name,action=0x7fff0000,args=()):
  nr=sc.seccomp_syscall_resolve_name(name.encode())
  if nr<0:return
  ar=(Arg*len(args))(*args);assert sc.seccomp_rule_add_array(ctx,action,nr,len(args),ar)==0,name
 allowed='''read write readv writev pread64 pwrite64 preadv pwritev preadv2 pwritev2 close close_range lseek
 open openat openat2 creat stat lstat fstat newfstatat statx access faccessat faccessat2 readlink readlinkat
 getdents getdents64 getcwd chdir fchdir mkdir mkdirat rmdir unlink unlinkat rename renameat renameat2
 link linkat symlink symlinkat truncate ftruncate fallocate fsync fdatasync sync syncfs sync_file_range
 chmod fchmod fchmodat fchmodat2 chown fchown lchown fchownat umask utime utimes futimesat utimensat
 getxattr lgetxattr fgetxattr listxattr llistxattr flistxattr setxattr lsetxattr fsetxattr
 removexattr lremovexattr fremovexattr getxattrat listxattrat setxattrat removexattrat
 mmap mprotect munmap mremap brk madvise mlock munlock mlockall munlockall
 rt_sigaction rt_sigprocmask rt_sigreturn rt_sigpending rt_sigtimedwait rt_sigsuspend sigaltstack
 exit exit_group fork vfork execve execveat chroot wait4 waitid getpid getppid gettid
 getuid geteuid getgid getegid getresuid getresgid getgroups setuid setgid setreuid setregid
 setresuid setresgid setfsuid setfsgid setgroups capget capset getpgrp getpgid setpgid setsid getsid
 kill tkill tgkill getrlimit setrlimit prlimit64 getrusage times clock_gettime clock_getres
 clock_nanosleep nanosleep gettimeofday time alarm setitimer getitimer
 sched_yield sched_getaffinity sched_setaffinity sched_getparam sched_getscheduler sched_get_priority_max sched_get_priority_min
 futex futex_waitv set_robust_list get_robust_list set_tid_address rseq membarrier
 pipe pipe2 dup dup2 dup3 fcntl poll ppoll select pselect6 epoll_create epoll_create1 epoll_ctl epoll_wait epoll_pwait epoll_pwait2
 eventfd eventfd2 signalfd signalfd4 timerfd_create timerfd_settime timerfd_gettime
 sendto recvfrom sendmsg recvmsg sendmmsg recvmmsg bind connect listen accept accept4 shutdown getsockname getpeername setsockopt getsockopt
 uname sysinfo getrandom arch_prctl restart_syscall statfs fstatfs flock fadvise64 readahead sendfile copy_file_range splice tee vmsplice
 '''.split()
 for name in allowed:rule(name)
 # Namespace flags are scalar in clone on the tested x86_64 ABI. clone3 carries
 # an opaque userspace structure, so return ENOSYS for libc's ordinary fallback.
 mask=0x7e020080;rule('clone',args=[Arg(0,7,mask,0)])
 rule('clone3',0x50000|errno.ENOSYS)
 for family in (socket.AF_UNIX,socket.AF_INET,socket.AF_INET6):rule('socket',args=[Arg(0,4,family,0)])
 rule('socketpair',args=[Arg(0,4,socket.AF_UNIX,0)])
 for op in (3,7,8,15,16,21,23,24,27,28,38,39,47):rule('prctl',args=[Arg(0,4,op,0)])
 for op in (0x5401,0x802c542a,0x540f,0x5413,0x541b,0x5451,0x5450,0x80086601):rule('ioctl',args=[Arg(1,4,op,0)])
 assert sc.seccomp_load(ctx)==0;sc.seccomp_release(ctx);emit('default_deny_loaded',status=status(),allowed_syscalls=allowed)
def child(mode,*args,drop=False):
 cmd=['python3','/probe.py',mode,*map(str,args)]
 if drop:cmd=['setpriv','--bounding-set=-sys_admin','--inh-caps=-sys_admin','--ambient-caps=-sys_admin',*cmd]
 r=subprocess.run(cmd,check=False,pass_fds=tuple(int(a) for a in args) if mode=='broker-client' else ())
 if r.returncode:
  if mode=='restricted':print('\n'.join(l for l in P('/image/cap-review/trace').read_text().splitlines() if 'EPERM' in l or "can't open" in l),flush=True)
  r.check_returncode()
 return r

def no_cap():
 st=status();assert all(not int(st[k],16)&(1<<21) for k in ('CapEff','CapPrm','CapBnd','CapInh','CapAmb')),st
 p='/image/cap-review/label';assert os.getxattr(p,'security.selinux')==LABEL
 assert 'security.selinux' in os.listxattr(p)
 must_errno(lambda:os.setxattr(p,'security.selinux',b'other\0'),errno.EPERM)
 must_errno(lambda:os.removexattr(p,'security.selinux'),errno.EPERM)
 fd=os.open(p,os.O_RDWR)
 assert os.getxattr(fd,'security.selinux')==LABEL
 must_errno(lambda:os.setxattr(fd,'security.selinux',LABEL),errno.EPERM)
 must_errno(lambda:os.removexattr(fd,'security.selinux'),errno.EPERM);os.close(fd)
 os.setxattr(p,'user.note',b'allowed');assert os.getxattr(p,'user.note')==b'allowed'
 emit('no_cap_reads_work_mutations_denied',status=st)

def broker_client(fd):
 st=status();assert not int(st['CapBnd'],16)&(1<<21),st
 s=socket.socket(fileno=int(fd));p='/image/cap-review/broker-file'
 for op,value in [('set','broker_label'),('remove','')]:
  s.send(json.dumps({'op':op,'value':value}).encode());assert s.recv(100)==b'ok'
  if op=='set':assert os.getxattr(p,'security.selinux')==value.encode()
  else:must_errno(lambda:os.getxattr(p,'security.selinux'),errno.ENODATA)
 must_errno(lambda:os.setxattr(p,'security.selinux',LABEL),errno.EPERM)
 emit('cooperative_broker_client',status=st)

def broker():
 p='/image/cap-review/broker-file';fd=os.open(p,os.O_CREAT|os.O_RDWR,0o600)
 a,b=socket.socketpair(socket.AF_UNIX,socket.SOCK_SEQPACKET)
 c=subprocess.Popen(['setpriv','--bounding-set=-sys_admin','--inh-caps=-sys_admin','--ambient-caps=-sys_admin','python3','/probe.py','broker-client',str(b.fileno())],pass_fds=[b.fileno()]);b.close();a.settimeout(5)
 try:
  for _ in range(2):
   request=json.loads(a.recv(1024));assert set(request)=={'op','value'}
   assert isinstance(request['value'],str) and len(request['value'])<256
   if request['op']=='set':os.setxattr(fd,'security.selinux',request['value'].encode())
   elif request['op']=='remove':os.removexattr(fd,'security.selinux')
   else:raise AssertionError(request)
   a.send(b'ok')
  assert c.wait(timeout=5)==0
 finally:
  a.close();os.close(fd)
  if c.poll() is None:c.kill();c.wait()
 emit('fixed_fd_broker_mechanism_only')

def unrestricted():
 p='/tmp/cap-review-mount';P(p).mkdir(exist_ok=True)
 libc.mount.argtypes=[ctypes.c_char_p]*3+[ctypes.c_ulong,ctypes.c_char_p]
 assert libc.mount(b'tmpfs',p.encode(),b'tmpfs',6,b'size=64k')==0,ctypes.get_errno()
 P(p+'/owned').write_text('owned disposable namespace control')
 assert libc.umount2(p.encode(),0)==0,ctypes.get_errno()
 assert libc.unshare(0)==0
 emit('unfiltered_owned_tmpfs_control',status=status())

def restricted():
 restrict()
 # No host targets: these are dummy descriptors, the caller's own namespace,
 # or a tiny owned mountpoint. Denial is at seccomp entry, before argument use.
 tests=[('unshare',272,0),('setns',308,-1,0),('mount',165,0,0,0,0,0),('umount2',166,0,0),('pivot_root',155,0,0),
 ('fsopen',430,ctypes.c_char_p(b'tmpfs'),0),('fsconfig',431,-1,0,0,0,0),('fsmount',432,-1,0,0),('open_tree',428,-100,ctypes.c_char_p(b'/image'),1),('move_mount',429,-1,0,-1,0,0),('mount_setattr',442,-1,0,0,0,0),
 ('bpf',321,-1,0,0),('perf_event_open',298,0,0,-1,-1,0),('userfaultfd',323,0),('io_uring_setup',425,0,0),
 ('ptrace',101,-1,0,0,0),('process_vm_readv',310,os.getpid(),0,0,0,0,0),('pidfd_getfd',438,-1,-1,0),('open_by_handle_at',304,-1,0,0),
 ('keyctl',250,-1,0,0,0,0),('sethostname',170,ctypes.c_char_p(b'owned'),5),('ioctl_not_allowlisted',16,-1,0x40086602,0),('clone_userns',56,0x10000000|17,0,0,0,0)]
 for name,nr,*args in tests:syscheck(name,nr,*args)
 syscheck('clone3_fallback',435,0,0,expected=errno.ENOSYS)
 # Ordinary process creation and metadata remain functional under the filter.
 subprocess.run(['bash','-eu','-c','mkdir /image/cap-review/work; printf data > /image/cap-review/work/a; cp /image/cap-review/work/a /image/cap-review/work/b; ln /image/cap-review/work/b /image/cap-review/work/c; chmod 640 /image/cap-review/work/a; chown 123:456 /image/cap-review/work/a'],check=True)
 p='/image/cap-review/label';os.setxattr(p,'security.selinux',LABEL);assert os.getxattr(p,'security.selinux')==LABEL
 os.removexattr(p,'security.selinux');os.setxattr(p,'security.selinux',LABEL)
 import checks
 checks.metadata('/image',0,'same-userns-control')
 # A new exec still has the filter; no privileged operations are necessary.
 subprocess.run(['python3','/probe.py','inherited'],check=True)
 emit('restricted_metadata_and_process_workload',status=status())

def run():
 P('/image/cap-review').mkdir();P('/image/cap-review/label').write_text('owned')
 os.setxattr('/image/cap-review/label','security.selinux',LABEL)
 child('nocap',drop=True);broker();child('unrestricted');child('restricted')
 if P('/rpms').exists():child('package')
 emit('ALL_MECHANISM_PROBES_PASS')

def run_lsm(case="normal"):
 if case in ("lsm-off","lsm-foreign"):
  restrict()
  p="/image/lsm-review/label";assert os.getxattr(p,"security.selinux")==LABEL
  must_errno(lambda:os.setxattr(p,"security.selinux",LABEL),errno.EPERM)
  must_errno(lambda:os.removexattr(p,"security.selinux"),errno.EPERM)
  emit("LSM_POLICY_REMOVAL_DENIES" if case=="lsm-off" else "LSM_OTHER_NAMESPACE_DENIED",status=status());return
 st=status();assert all(not int(st[k],16)&(1<<21) for k in ('CapEff','CapPrm','CapBnd','CapInh','CapAmb')),st
 restrict()
 P('/image/lsm-review').mkdir();p='/image/lsm-review/label';P(p).write_text('owned')
 os.setxattr(p,'security.selinux',LABEL,os.XATTR_CREATE);assert os.getxattr(p,'security.selinux')==LABEL
 must_errno(lambda:os.setxattr(p,'security.selinux',LABEL,os.XATTR_CREATE),errno.EEXIST)
 fd=os.open(p,os.O_RDWR);os.setxattr(fd,'security.selinux',b'replaced',os.XATTR_REPLACE);assert os.getxattr(fd,'security.selinux')==b'replaced'
 os.link(p,p+'-hardlink');assert os.getxattr(p+'-hardlink','security.selinux')==b'replaced'
 os.removexattr(fd,'security.selinux');must_errno(lambda:os.getxattr(p,'security.selinux'),errno.ENODATA);os.close(fd)
 os.setxattr(p,'security.selinux',LABEL)
 os.symlink('label',p+'-symlink');os.setxattr(p+'-symlink','security.selinux',b'link-label',follow_symlinks=False)
 assert os.getxattr(p+'-symlink','security.selinux',follow_symlinks=False)==b'link-label'
 assert os.getxattr(p,'security.selinux')==LABEL
 for name in ('security.selinuxX','security.selinux.extra','security.selinux'+'x'*100):
  must_errno(lambda:os.setxattr(p,name,b'no'),errno.EPERM)
  must_errno(lambda:os.removexattr(p,name),errno.EPERM)
 emit('LSM_EXACT_ATTRIBUTE_PASS')
 must_errno(lambda:os.setxattr(p,'security.other',b'no'),errno.EPERM)
 must_errno(lambda:os.setxattr(p,'trusted.other',b'no'),errno.EPERM)
 other='/tmp/lsm-negative';P(other).write_text('other filesystem')
 must_errno(lambda:os.setxattr(other,'security.selinux',LABEL),errno.EPERM)
 own='/image/lsm-review/nonroot-owned';P(own).write_text('owned');os.chown(own,1234,1234);os.chmod(own,0o600)
 subprocess.run(['python3','-c',"import os; os.setgid(1234); os.setuid(1234); p='/image/lsm-review/nonroot-owned';open(p,'w').write('ordinary write allowed');\ntry: os.setxattr(p,'security.selinux',b'no')\nexcept OSError as e: assert e.errno in (1,13)\nelse: raise AssertionError('nonroot label accepted')"],check=True)
 import checks;checks.metadata('/image',0,'same-userns-control')
 emit('LSM_NO_ADMIN_METADATA_PASS',status=status())
 if case=='cleanup-failure':
  child=os.fork()
  if child==0:
   while True:time.sleep(1)
  emit('CLEANUP_FAILURE_READY')
  while True:time.sleep(1)
 if P('/rpms').exists():package(filtered=True)

def package(filtered=False):
 if not filtered:restrict()
 P('/image/os').mkdir()
 argv=['dnf5','--no-plugins',"--disable-repo=*",'--installroot=/image/os','--releasever=44','--use-host-config','--setopt=install_weak_deps=False','--no-gpgchecks','-y','install']
 subprocess.run(argv+[str(p) for p in sorted(P('/rpms').glob('*.rpm'))],check=True)
 subprocess.run(['rpm','--root','/image/os','-q','bash','coreutils','python3'],check=True)
 subprocess.run(['chroot','/image/os','/bin/bash','-eu','-c','printf package-root-ok; python3 -c "print(123)"'],check=True)
 os.setxattr('/image/os/usr/bin/bash','security.selinux',LABEL)
 assert os.getxattr('/image/os/usr/bin/bash','security.selinux')==LABEL
 emit('FEDORA_OFFLINE_BOOTSTRAP_PASS',status=status())

if __name__=='__main__':
 mode=sys.argv[1]
 if mode=='package':package()
 elif mode=='nocap':no_cap()
 elif mode=='broker-client':broker_client(sys.argv[2])
 elif mode=='unrestricted':unrestricted()
 elif mode=='restricted':restricted()
 elif mode=='inherited':syscheck('exec_inherits_filter',272,0)
 else:raise ValueError(mode)
