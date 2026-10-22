
import json,os,socket
from pathlib import Path
status=dict(line.split(':',1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
for key in ('CapInh','CapPrm','CapEff','CapBnd','CapAmb'): assert int(status[key],16)==0,(key,status[key])
assert status['NoNewPrivs'].strip()=='1'
assert os.getcwd()=='/'
assert 'SUPERVISOR_SENTINEL' not in os.environ
assert not Path('/sys/fs/cgroup').exists()
assert not Path('/var/tmp/conglobate-uml-resources-filter-v1/supervisor-secret').exists()
assert not Path('/home').exists()
fds={}
for p in Path('/proc/self/fd').iterdir():
 try:fds[p.name]=os.readlink(p)
 except FileNotFoundError:pass
assert not any('supervisor-secret' in s for s in fds.values())
assert set(os.listdir('/sys'))==set() if Path('/sys').exists() else True
interfaces=Path('/proc/net/dev').read_text()
assert all('lo:' in s for s in interfaces.splitlines()[2:])
for family,address in [(socket.AF_INET,('127.0.0.1',42111)),(socket.AF_UNIX,'\0conglobate-owned-3512326'),(socket.AF_UNIX,'/var/tmp/conglobate-uml-resources-filter-v1/control.sock')]:
 s=socket.socket(family,socket.SOCK_STREAM);s.settimeout(.2)
 try:s.connect(address)
 except OSError:pass
 else:raise AssertionError(('unexpected supervisor socket connection',address))
 finally:s.close()
print(json.dumps({'status':{k:status[k].strip() for k in ('CapEff','CapBnd','NoNewPrivs')},'fds':fds,'environment':dict(os.environ),'interfaces':interfaces,'proc_pids':[p.name for p in Path('/proc').iterdir() if p.name.isdigit()]}),flush=True)
print('RESOURCE_BOUNDARY_OK',flush=True)
