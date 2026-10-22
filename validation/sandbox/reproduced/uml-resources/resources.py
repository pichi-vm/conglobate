#!/usr/bin/env python3
"""Bounded sandbox resource tests in a delegated, caller-owned cgroup subtree."""
import argparse,errno,json,os,selectors,shutil,socket,subprocess,sys,tempfile,time
from pathlib import Path
import probe

# Only the trusted launcher sees this path, before entering the filesystem view.
if len(sys.argv)>1 and sys.argv[1]=='--enter':
    Path(sys.argv[2],'cgroup.procs').write_text(str(os.getpid()))
    os.execvp(sys.argv[3],sys.argv[3:])

CHECK=r'''
import json,os,socket
from pathlib import Path
status=dict(line.split(':',1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
for key in ('CapInh','CapPrm','CapEff','CapBnd','CapAmb'): assert int(status[key],16)==0,(key,status[key])
assert status['NoNewPrivs'].strip()=='1'
assert os.getcwd()=='/'
assert 'SUPERVISOR_SENTINEL' not in os.environ
assert not Path('/sys/fs/cgroup').exists()
assert not Path('@SECRET@').exists()
assert not Path('/home').exists()
fds={}
for p in Path('/proc/self/fd').iterdir():
 try:fds[p.name]=os.readlink(p)
 except FileNotFoundError:pass
assert not any('supervisor-secret' in s for s in fds.values())
assert set(os.listdir('/sys'))==set() if Path('/sys').exists() else True
interfaces=Path('/proc/net/dev').read_text()
assert all('lo:' in s for s in interfaces.splitlines()[2:])
for family,address in [(socket.AF_INET,('127.0.0.1',@PORT@)),(socket.AF_UNIX,'\0@ABSTRACT@'),(socket.AF_UNIX,'@SOCKET@')]:
 s=socket.socket(family,socket.SOCK_STREAM);s.settimeout(.2)
 try:s.connect(address)
 except OSError:pass
 else:raise AssertionError(('unexpected supervisor socket connection',address))
 finally:s.close()
print(json.dumps({'status':{k:status[k].strip() for k in ('CapEff','CapBnd','NoNewPrivs')},'fds':fds,'environment':dict(os.environ),'interfaces':interfaces,'proc_pids':[p.name for p in Path('/proc').iterdir() if p.name.isdigit()]}),flush=True)
print('RESOURCE_BOUNDARY_OK',flush=True)
'''

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,required=True);a=ap.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
    cg=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().strip().split('::')[1].lstrip('/')
    manager=cg/'manager';manager.mkdir();(manager/'cgroup.procs').write_text(str(os.getpid()))
    (cg/'cgroup.subtree_control').write_text('+memory +pids')
    secret=out/'supervisor-secret';secret.write_text('intact\n'); fd=os.open(secret,os.O_RDONLY);os.set_inheritable(fd,True)
    tcp=socket.socket();tcp.bind(('127.0.0.1',0));tcp.listen();port=tcp.getsockname()[1]
    abstract='conglobate-owned-'+str(os.getpid());unix=socket.socket(socket.AF_UNIX);unix.bind('\0'+abstract);unix.listen()
    path=out/'control.sock';local=socket.socket(socket.AF_UNIX);local.bind(str(path));local.listen()
    # Positive controls: every owned listener is reachable from the supervisor.
    for listener,address in [(tcp,('127.0.0.1',port)),(unix,'\0'+abstract),(local,str(path))]:
        client=socket.socket(listener.family);client.settimeout(1);client.connect(address)
        accepted,_=listener.accept();accepted.close();client.close()
    results=[]
    def execute(name,cmd,limit=256*1024*1024,tasks=32,deadline=8,logcap=1024*1024):
        group=cg/name;group.mkdir();(group/'memory.max').write_text(str(limit));(group/'memory.swap.max').write_text('0');(group/'memory.oom.group').write_text('1');(group/'pids.max').write_text(str(tasks))
        started=time.monotonic();output=bytearray();reason='exit'
        env=dict(os.environ,SUPERVISOR_SENTINEL='must not propagate')
        p=subprocess.Popen([sys.executable,str(Path(__file__).resolve()),'--enter',str(group),*cmd],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,env=env,close_fds=True)
        try:
            with selectors.DefaultSelector() as sel:
                sel.register(p.stdout,selectors.EVENT_READ)
                while sel.get_map():
                    if time.monotonic()-started>deadline:reason='timeout';break
                    for key,_ in sel.select(.05):
                        chunk=os.read(key.fd,65536)
                        if not chunk:sel.unregister(key.fileobj);continue
                        output.extend(chunk)
                    if len(output)>logcap:reason='log-limit';break
        finally:
            (group/'cgroup.kill').write_text('1')
            p.wait(timeout=5);p.stdin.close();p.stdout.close()
            until=time.monotonic()+5
            while 'populated 1' in (group/'cgroup.events').read_text():
                if time.monotonic()>until:raise RuntimeError('cgroup remains populated; retained '+str(group))
                time.sleep(.01)
        row={'name':name,'command':cmd,'reason':reason,'returncode':p.returncode,'seconds':time.monotonic()-started,'cgroup_events':(group/'cgroup.events').read_text(),'memory_events':(group/'memory.events').read_text(),'memory_peak':(group/'memory.peak').read_text(),'pids_events':(group/'pids.events').read_text(),'output_bytes':len(output),'limits':{'memory':limit,'pids':tasks}}
        (out/(name+'.log')).write_bytes(output);(out/(name+'.json')).write_text(json.dumps(row,indent=2)+'\n')
        group.rmdir();results.append(row);print(json.dumps({k:v for k,v in row.items() if k!='command'}),flush=True)
        return row,bytes(output)
    def native(name,script,**limits):
        f=out/(name+'.py');f.write_text(script)
        cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--setenv','PATH','/usr/bin',
             '--ro-bind','/usr','/usr','--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64','--proc','/proc','--dev','/dev',
             '--size',str(8*1024*1024),'--tmpfs','/work','--ro-bind',str(f),'/probe.py','--chdir','/','/usr/bin/python3','-I','/probe.py']
        return execute(name,cmd,**limits)
    script=CHECK.replace('@SECRET@',str(secret)).replace('@PORT@',str(port)).replace('@ABSTRACT@',abstract).replace('@SOCKET@',str(path))
    r,log=native('visibility',script);r['case_pass']=r['returncode']==0 and b'RESOURCE_BOUNDARY_OK' in log
    r,log=native('memory',"x=bytearray(128*1024*1024)\nprint('unexpected allocation success')\n",limit=64*1024*1024)
    r['case_pass']=r['returncode']!=0 and any(int(line.split()[1])>0 for line in r['memory_events'].splitlines() if line.startswith('oom_kill '))
    r,log=native('tasks',"import subprocess\nchildren=[]\ntry:\n for i in range(32): children.append(subprocess.Popen(['/usr/bin/sleep','10']))\nexcept BlockingIOError: print('PID_LIMIT_OK',flush=True)\n",tasks=16,deadline=3)
    r['case_pass']='max 0' not in r['pids_events'] and b'PID_LIMIT_OK' in log
    r,log=native('storage',"import errno\ntry:\n with open('/work/finite','wb') as f: f.write(b'x'*(16*1024*1024))\nexcept OSError as e:\n assert e.errno==errno.ENOSPC\n print('ENOSPC_OK',flush=True)\nelse: raise AssertionError('expected full tmpfs')\n")
    r['case_pass']=r['returncode']==0 and b'ENOSPC_OK' in log
    r,log=native('logs',"import os,time\nfor i in range(128): os.write(1,b'x'*4096)\ntime.sleep(2)\n",logcap=64*1024)
    r['case_pass']=r['reason']=='log-limit' and r['output_bytes']<=128*1024
    # Actual UML in the same cgroup ownership scheme; only benign sleep/poweroff.
    for name,body,deadline in [('uml-normal','busybox sleep 20 &\necho COMPLETE\nbusybox poweroff -f\n',8),('uml-timeout','busybox sleep 20\n',3)]:
        runtime=Path(tempfile.mkdtemp(prefix='conglobate-uml-cgroup-',dir='/dev/shm'))
        init=out/(name+'.cpio');init.write_bytes(probe.archive('#!/bin/sh\nexport PATH=/bin\necho READY\n'+body))
        cmd=json.loads(Path('/var/tmp/conglobate-uml-validation-v4/ext4-write.json').read_text())['command']
        for i,value in enumerate(cmd):
            if value.endswith('ext4-write.cpio'):cmd[i]=str(init)
            if value.startswith('/dev/shm/conglobate-uml-confined-'):cmd[i]=str(runtime)
        r,log=execute(name,cmd,limit=768*1024*1024,tasks=48,deadline=deadline)
        r['case_pass']=(r['returncode']==0 and b'COMPLETE' in log) if name=='uml-normal' else (r['reason']=='timeout' and b'READY' in log)
        shutil.rmtree(runtime)
    assert secret.read_text()=='intact\n'
    os.close(fd);tcp.close();unix.close();local.close();path.unlink()
    for r in results:(out/(r['name']+'.json')).write_text(json.dumps(r,indent=2)+'\n')
    (out/'results.json').write_text(json.dumps({'results':results,'pass':all(r['case_pass'] for r in results)},indent=2)+'\n')
    print('SUITE_PASS',all(r['case_pass'] for r in results),flush=True)
if __name__=='__main__':main()
