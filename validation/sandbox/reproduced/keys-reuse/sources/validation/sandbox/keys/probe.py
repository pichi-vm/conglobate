import ctypes,errno,os,subprocess,sys
libc=ctypes.CDLL(None,use_errno=True)
libc.syscall.restype=ctypes.c_long
# x86_64-only fixture, matching the QEMU/host architecture used by guest.py.
KEYCTL=250;ADD_KEY=248;JOIN_SESSION=1;READ=11;SETPERM=5
if len(sys.argv)==1:
 ring=libc.syscall(KEYCTL,JOIN_SESSION,ctypes.c_void_p())
 assert ring>0,(ring,ctypes.get_errno())
 secret=b'supervisor-keyring-canary'
 key=libc.syscall(ADD_KEY,ctypes.c_char_p(b'user'),ctypes.c_char_p(b'conglobate-validation'),ctypes.c_char_p(secret),len(secret),ring)
 assert key>0,(key,ctypes.get_errno())
 assert libc.syscall(KEYCTL,SETPERM,key,0x3f3f0000)==0
 subprocess.run(['unshare','--user','--mount','--pid','--net','--ipc','--uts','--fork',
                 '--map-users','0:100000:65536','--map-groups','0:100000:65536',
                 '--setuid','0','--setgid','0','python3','/probe.py',str(key)],check=True)
elif len(sys.argv)==2 and sys.argv[1].isdigit():
 key=int(sys.argv[1]);buf=ctypes.create_string_buffer(128)
 n=libc.syscall(KEYCTL,READ,key,buf,128)
 if n>=0:
  assert buf.raw[:n]==b'supervisor-keyring-canary'
  print('INHERITED_SESSION_KEY_VISIBLE',flush=True)
 else:
  print('INHERITED_SESSION_KEY_DENIED',ctypes.get_errno(),flush=True)
 ring=libc.syscall(KEYCTL,JOIN_SESSION,ctypes.c_void_p())
 assert ring>0,(ring,ctypes.get_errno())
 n=libc.syscall(KEYCTL,READ,key,buf,128)
 assert n<0 and ctypes.get_errno() in (errno.EACCES,errno.EPERM,errno.ENOKEY),(n,ctypes.get_errno())
 print('NEW_SESSION_KEYRING_BLOCKS_SUPERVISOR_KEY_PASS',flush=True)


# Same-host-UID worker ownership is different from inherited possession.
if len(sys.argv)==1:
 os.makedirs('/work/worker',exist_ok=True)
 os.chown('/work/worker',100000,100000)
 worker=subprocess.Popen(['setpriv','--reuid=100000','--regid=100000','--clear-groups',
                          'python3','/probe.py','worker'],stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE,text=True)
 try:
  key=int(worker.stdout.readline())
  assert key>0
  for uid,expected in ((100000,'readable'),(200000,'denied')):
   subprocess.run(['unshare','--user','--mount','--pid','--net','--ipc','--uts','--fork',
                   '--map-users',f'0:{uid}:65536','--map-groups',f'0:{uid}:65536',
                   '--setuid','0','--setgid','0','python3','/probe.py','owner-check',str(key),expected],
                  check=True,timeout=10)
 finally:
  worker.communicate('x',timeout=5)
 assert worker.returncode==0
 print('WORKER_KEY_UID_MATRIX_PASS',flush=True)
elif sys.argv[1]=='worker':
 # Anonymous session isolates possession; per-UID ring retains the test key.
 session=libc.syscall(KEYCTL,JOIN_SESSION,ctypes.c_void_p())
 assert session>0
 user_ring=libc.syscall(KEYCTL,0,-4,1)
 assert user_ring>0,(user_ring,ctypes.get_errno())
 assert libc.syscall(KEYCTL,8,user_ring,session)==0,("link",ctypes.get_errno())
 payload=b'worker-owned-canary'
 key=libc.syscall(ADD_KEY,ctypes.c_char_p(b'user'),ctypes.c_char_p(b'worker-key'),
                  ctypes.c_char_p(payload),len(payload),user_ring)
 assert key>0
 # User permissions only: readable by its owner, no possessor/group/other bits.
 assert libc.syscall(KEYCTL,SETPERM,key,0x003f0000)==0,("setperm",ctypes.get_errno())
 print(key,flush=True)
 assert sys.stdin.read(1)=='x'
 buf=ctypes.create_string_buffer(128)
 count=libc.syscall(KEYCTL,READ,key,buf,128)
 assert count==len(payload) and buf.raw[:count]==payload
 assert libc.syscall(KEYCTL,3,key)==0  # Revoke our disposable canary.
elif sys.argv[1]=='owner-check':
 key=int(sys.argv[2]);expected=sys.argv[3]
 assert libc.syscall(KEYCTL,JOIN_SESSION,ctypes.c_void_p())>0
 buf=ctypes.create_string_buffer(128)
 count=libc.syscall(KEYCTL,READ,key,buf,128)
 if expected=='readable':
  assert count==len(b'worker-owned-canary') and buf.raw[:count]==b'worker-owned-canary'
  print('FRESH_SESSION_SAME_UID_WORKER_KEY_VISIBLE',flush=True)
 else:
  assert count<0 and ctypes.get_errno() in (errno.EACCES,errno.EPERM,errno.ENOKEY)
  print('FRESH_SESSION_DIFFERENT_UID_WORKER_KEY_DENIED',flush=True)
