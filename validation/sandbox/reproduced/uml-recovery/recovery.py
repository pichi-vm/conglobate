#!/usr/bin/env python3
"""Owned-process loss/recovery tests; entire UML remains outside supervisor trust."""
import argparse,fcntl,hashlib,json,os,shutil,signal,subprocess,sys,tempfile,time
from pathlib import Path
import probe
SELF=Path(__file__).resolve()
def atomic(path,value):
    tmp=path.with_suffix('.tmp');tmp.write_text(json.dumps(value)+'\n');os.replace(tmp,path)

if len(sys.argv)>1 and sys.argv[1]=='--worker':
    Path(sys.argv[2],'cgroup.procs').write_text(str(os.getpid()))
    os.execvp(sys.argv[3],sys.argv[3:])
if len(sys.argv)>1 and sys.argv[1]=='--owner':
    d=Path(sys.argv[2]);cfg=json.loads((d/'config.json').read_text())
    fd=os.open(d/'lease',os.O_CREAT|os.O_RDWR,0o600);fcntl.flock(fd,fcntl.LOCK_EX)
    atomic(d/'state.json',{'state':'Building','nonce':cfg['nonce']})
    atomic(d/'owner-ready.json',{'pid':os.getpid()})
    while not (d/'start').exists():time.sleep(.01)
    filterfd=os.open('/var/tmp/conglobate-uml-filter.bpf',os.O_RDONLY)
    cmd=cfg['command'];cmd=[cmd[0],'--seccomp',str(filterfd),*cmd[1:]]
    with (d/'uml.log').open('wb') as log:
        p=subprocess.Popen([sys.executable,str(SELF),'--worker',cfg['cgroup'],*cmd],stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,pass_fds=(filterfd,))
        os.close(filterfd)
        code=p.wait(timeout=25);p.stdin.close()
    atomic(d/'owner-result.json',{'returncode':code})
    # Intentional pre-publication barrier. Completion requires supervisor action;
    # an inner COMPLETE line alone cannot write that state or release this lease.
    while not (d/'finish').exists():time.sleep(.01)
    raise SystemExit(0 if code==0 else 1)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,required=True);a=ap.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
    parent=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().strip().split('::')[1].lstrip('/')
    manager=parent/'manager';manager.mkdir();(manager/'cgroup.procs').write_text(str(os.getpid()))
    (parent/'cgroup.subtree_control').write_text('+memory +pids')
    rows=[]
    for stage in ['before-launch','running','after-exit']:
        d=out/stage;d.mkdir();group=parent/stage;group.mkdir()
        (group/'memory.max').write_text(str(768*1024*1024));(group/'memory.swap.max').write_text('0');(group/'pids.max').write_text('48')
        cgfd=os.open(group,os.O_DIRECTORY);identity=os.fstat(cgfd).st_ino
        def cgwrite(name,value):
            fd=os.open(name,os.O_WRONLY,dir_fd=cgfd)
            try:os.write(fd,value.encode())
            finally:os.close(fd)
        def cgread(name):
            fd=os.open(name,os.O_RDONLY,dir_fd=cgfd)
            try:return os.read(fd,4096).decode()
            finally:os.close(fd)
        runtime=Path(tempfile.mkdtemp(prefix='conglobate-uml-recovery-',dir='/dev/shm'))
        script='#!/bin/sh\nexport PATH=/bin\necho READY\n'
        script+=('echo COMPLETE\nbusybox poweroff -f\n' if stage=='after-exit' else 'busybox sleep 20\n')
        init=d/'initrd';init.write_bytes(probe.archive(script));(d/'init').write_text(script)
        cmd=json.loads(Path('/var/tmp/conglobate-uml-validation-v4/ext4-write.json').read_text())['command']
        # Use a fresh disposable output, not a filesystem image reused by other tests.
        disk=d/'output';disk.write_bytes(b'owned output\n')
        for i,v in enumerate(cmd):
            if v.endswith('ext4-write.cpio'):cmd[i]=str(init)
            if v.endswith('ext4.img'):cmd[i]=str(disk)
            if v.startswith('/dev/shm/conglobate-uml-confined-'):cmd[i]=str(runtime)
        nonce=os.urandom(16).hex();atomic(d/'config.json',{'nonce':nonce,'cgroup':str(group),'inode':identity,'command':cmd})
        owner=subprocess.Popen([sys.executable,str(SELF),'--owner',str(d)])
        ownerfd=os.pidfd_open(owner.pid)
        def until(predicate):
            deadline=time.monotonic()+15
            while not predicate():
                if time.monotonic()>deadline:raise TimeoutError(stage)
                time.sleep(.01)
        try:
            until(lambda:(d/'owner-ready.json').exists())
            lease=os.open(d/'lease',os.O_RDWR)
            try:
                try:fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
                except BlockingIOError:lease_excluded=True
                else:raise AssertionError('live owner did not hold lease')
                if stage!='before-launch':
                    (d/'start').touch()
                    until(lambda:(d/'uml.log').exists() and b'\nREADY\r\n' in (d/'uml.log').read_bytes())
                if stage=='after-exit':until(lambda:(d/'owner-result.json').exists())
                populated_before=cgread('cgroup.events')
                signal.pidfd_send_signal(ownerfd,signal.SIGKILL);owner.wait(timeout=5)
                until(lambda:owner.poll() is not None)
                fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
                # Crash recovery uses retained authoritative cgroup handle. An
                # unlocked lease or numeric PID alone never proves cleanup.
                cgwrite('cgroup.kill','1');until(lambda:'populated 0' in cgread('cgroup.events'))
                assert os.stat(group).st_ino==identity
                record=json.loads((d/'state.json').read_text());assert record=={'state':'Building','nonce':nonce}
                atomic(d/'state.json',{'state':'Interrupted','nonce':nonce,'cgroup_empty':True})
                assert not (d/'published').exists()
                row={'stage':stage,'owner_returncode':owner.returncode,'lease_excluded_while_live':lease_excluded,'before':populated_before,'after':cgread('cgroup.events'),'state':json.loads((d/'state.json').read_text()),'pass':True}
                if stage=='running':assert 'populated 1' in populated_before
                if stage=='after-exit':assert json.loads((d/'owner-result.json').read_text())['returncode']==0
                rows.append(row);print(json.dumps(row),flush=True)
            finally:os.close(lease)
        finally:
            if owner.poll() is None:signal.pidfd_send_signal(ownerfd,signal.SIGKILL);owner.wait(timeout=5)
            os.close(ownerfd);cgwrite('cgroup.kill','1')
            until(lambda:'populated 0' in cgread('cgroup.events'))
            os.close(cgfd);group.rmdir();shutil.rmtree(runtime)
    (out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
if __name__=='__main__':main()
