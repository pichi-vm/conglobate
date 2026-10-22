import ctypes,errno,os,socket,subprocess,sys
libc=ctypes.CDLL(None,use_errno=True)
key=0x43474c42
if len(sys.argv)>1:
 assert libc.semget(key,1,0)==-1 and ctypes.get_errno()==errno.ENOENT
 assert libc.shmget(key,4096,0)==-1 and ctypes.get_errno()==errno.ENOENT
 original=socket.gethostname();socket.sethostname(b'isolated-command')
 assert socket.gethostname()=='isolated-command'
 subprocess.run(['mount','-t','tmpfs','tmpfs','/dev/shm'],check=True)
 assert not os.path.exists('/dev/shm/supervisor-token')
 print('SYSV_IPC_UTS_PRIVATE_SHM_PASS',flush=True)
else:
 sem=libc.semget(key,1,0o1000|0o600);shm=libc.shmget(key,4096,0o1000|0o600)
 assert sem>=0 and shm>=0
 os.makedirs('/dev/shm',exist_ok=True);open('/dev/shm/supervisor-token','w').write('private')
 original=socket.gethostname()
 try:
  subprocess.run(['unshare','--ipc','--uts','--mount','--propagation','private','python3','/ipc.py','child'],check=True)
  assert socket.gethostname()==original
  assert open('/dev/shm/supervisor-token').read()=='private'
  assert libc.semget(key,1,0)==sem and libc.shmget(key,4096,0)==shm
  print('SUPERVISOR_IPC_UTS_UNCHANGED_PASS',flush=True)
 finally:
  assert libc.semctl(sem,0,0)==0
  assert libc.shmctl(shm,0,None)==0
