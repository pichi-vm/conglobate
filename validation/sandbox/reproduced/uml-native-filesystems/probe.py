#!/usr/bin/env python3
"""Benign confined UML filesystem and hostfs probes; no host mounts/root."""
import argparse, hashlib, json, os, re, selectors, shutil, signal, stat, subprocess, tempfile, time
from pathlib import Path

BASE = Path('/var/tmp/conglobate-debian-uml-boot-v3')

def cpio(files):
    data = bytearray()
    for ino, (name, mode, payload, major, minor) in enumerate(files + [('TRAILER!!!',0,b'',0,0)],1):
        name = name.encode()+b'\0'
        fields=[ino,mode,0,0,1,0,len(payload),0,0,major,minor,len(name),0]
        data += b'070701'+b''.join(f'{n:08x}'.encode() for n in fields)+name
        data += b'\0'*(-len(data)%4)
        data += payload
        data += b'\0'*(-len(data)%4)
    return data

def archive(script, extra_files=()):
    files=[]; added=set()
    def add(name,mode,payload=b'',major=0,minor=0):
        if name in added: return
        if '/' in name: add(name.rsplit('/',1)[0],stat.S_IFDIR|0o755)
        added.add(name); files.append((name,mode,payload,major,minor))
    for d in ['bin','dev','proc','sys','mnt','host','tmp']: add(d,stat.S_IFDIR|0o755)
    add('dev/console',stat.S_IFCHR|0o600,major=5,minor=1)
    add('dev/null',stat.S_IFCHR|0o666,major=1,minor=3)
    add('dev/ubda',stat.S_IFBLK|0o600,major=98,minor=0)
    add('bin/busybox',stat.S_IFREG|0o755,(BASE/'busybox').read_bytes())
    add('bin/sh',stat.S_IFLNK|0o777,b'busybox')
    for tool in ['setfattr','getfattr','setfacl','getfacl','setcap','getcap','chattr','lsattr']:
        path=Path(shutil.which(tool))
        add('bin/'+tool,stat.S_IFREG|0o755,path.read_bytes())
        deps=subprocess.check_output(['ldd',str(path)],text=True)
        for lib in re.findall(r'(/[^\s()]+)',deps):
            p=Path(lib); add(str(p).lstrip('/'),stat.S_IFREG|0o755,p.read_bytes())
    for name, mode, payload in extra_files: add(name, mode, payload)
    add('init',stat.S_IFREG|0o755,script.encode())
    return cpio(files)

INIT='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
busybox mount -t sysfs sysfs /sys
busybox mount -t securityfs securityfs /sys/kernel/security
test "$(busybox cat /sys/kernel/security/lsm)" = capability
echo READY
busybox mount -t hostfs -o / none /host
test ! -e /host@SENTINEL@
test ! -e /host/home
test ! -e /host/context
test -r /host/allowed
if echo wrong >> /host/allowed; then exit 31; fi
busybox cat /host/proc/self/status
busybox ls -l /host/proc/self/fd || true # diagnostic: proc FDs change during enumeration
busybox cat /host/proc/net/dev
busybox umount /host
echo HOSTFS_BOUNDARY_OK
busybox mount -t @FS@ /dev/ubda /mnt
@WORK@
busybox sync
busybox umount /mnt
echo COMPLETE
busybox poweroff -f
'''
WORK='''echo payload > /mnt/data
busybox chown 1234:2345 /mnt/data
busybox chmod 6751 /mnt/data
busybox ln /mnt/data /mnt/hard
busybox ln -s data /mnt/link
busybox mknod /mnt/null c 1 3
busybox truncate -s 16777216 /mnt/sparse
setfattr -n user.probe -v value /mnt/data
setfattr -n trusted.probe -v trusted /mnt/data
setfattr -n security.selinux -v foreign_u:object_r:foreign_image_t:s0 /mnt/data
setfacl -m u:3456:r-- /mnt/data
busybox cp /bin/busybox /mnt/capfile
setcap cap_net_bind_service=ep /mnt/capfile
echo flag > /mnt/flag
chattr +i /mnt/flag
lsattr /mnt/flag
chattr -i /mnt/flag
chattr +i /mnt/flag
echo append > /mnt/append
chattr +a /mnt/append
/bin/getfattr -d -m - /mnt/data
getfacl -n /mnt/data
getcap /mnt/capfile
busybox stat -c '%n %u %g %a %h %s %b' /mnt/data /mnt/hard /mnt/sparse /mnt/null
'''
VERIFY='''test "$(busybox cat /mnt/data)" = payload
test "$(busybox stat -c '%u:%g' /mnt/data)" = 1234:2345
test "$(busybox stat -c '%h' /mnt/data)" = 2
test "$(busybox readlink /mnt/link)" = data
test -c /mnt/null
test "$(busybox stat -c '%a' /mnt/data)" = 6751
lsattr /mnt/flag | busybox grep -- '----i'
lsattr /mnt/append | busybox grep -- '-----a'
test "$(/bin/getfattr --only-values -n security.selinux /mnt/data)" = foreign_u:object_r:foreign_image_t:s0
test "$(/bin/getfattr --only-values -n trusted.probe /mnt/data)" = trusted
getfacl -n /mnt/data | busybox grep 'user:3456:r--'
getcap /mnt/capfile | busybox grep cap_net_bind_service=ep
test "$(busybox stat -c %b /mnt/sparse)" -lt 32768
echo PERSISTENCE_OK
'''

def run(out,fs,phase,disk,sentinel):
    runtime=Path(tempfile.mkdtemp(prefix='conglobate-uml-confined-',dir='/dev/shm'))
    init=INIT.replace('@FS@',fs).replace('@SENTINEL@',str(sentinel)).replace('@WORK@',WORK if phase=='write' else VERIFY)
    if phase == 'verify': init=init.replace('mount -t '+fs+' /dev/ubda', 'mount -t '+fs+' -o ro /dev/ubda')
    stem=out/(fs+'-'+phase)
    stem.with_suffix('.init').write_text(init)
    archive_path=stem.with_suffix('.cpio'); archive_path.write_bytes(archive(init))
    cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL',
         '--clearenv','--setenv','PATH','/bin','--setenv','TMPDIR','/runtime','--setenv','HOME','/runtime',
         '--setenv','LC_ALL','C','--proc','/proc','--dev','/dev','--dir','/lib64',
         '--ro-bind','/lib64/libc.so.6','/lib64/libc.so.6',
         '--ro-bind','/lib64/ld-linux-x86-64.so.2','/lib64/ld-linux-x86-64.so.2',
         '--ro-bind',str(BASE/'linux.uml'),'/linux',
         '--ro-bind',str(archive_path),'/initrd', '--ro-bind',str(out/'allowed'),'/allowed',
         ('--ro-bind' if phase=='verify' else '--bind'),str(disk),'/disk','--bind',str(runtime),'/runtime','--chdir','/',
         '/linux','mem=256M','con=null','con0=fd:0,fd:1','ssl=null','lsm=capability',
         'panic=1','rdinit=/init','initrd=/initrd','uml_dir=/runtime','umid=probe',('ubd0r=/disk' if phase=='verify' else 'ubd0=/disk')]
    start=time.monotonic(); output=bytearray(); ready=None; error=None
    p=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
    try:
        with selectors.DefaultSelector() as sel:
            sel.register(p.stdout,selectors.EVENT_READ)
            while sel.get_map():
                if time.monotonic()-start>60: raise TimeoutError('60s deadline')
                for key,_ in sel.select(.1):
                    chunk=os.read(key.fd,65536)
                    if not chunk: sel.unregister(key.fileobj); continue
                    output.extend(chunk)
                    if ready is None and b'\nREADY\r\n' in output: ready=time.monotonic()-start
                    if len(output)>2*1024*1024: raise RuntimeError('2MiB log cap')
        p.wait(timeout=2)
    except Exception as exc: error=repr(exc)
    finally:
        try: os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError: pass
        p.wait(timeout=5); p.stdin.close(); p.stdout.close()
        shutil.rmtree(runtime)
    log=bytes(output); stem.with_suffix('.log').write_bytes(log)
    lines=log.decode(errors='replace').splitlines()
    row={'filesystem':fs,'phase':phase,'command':cmd,'returncode':p.returncode,'error':error,
         'seconds':time.monotonic()-start,'ready_seconds':ready,
         'pass': p.returncode==0 and error is None and 'COMPLETE' in lines and 'HOSTFS_BOUNDARY_OK' in lines,
         'initrd_sha256':hashlib.sha256(archive_path.read_bytes()).hexdigest()}
    stem.with_suffix('.json').write_text(json.dumps(row,indent=2)+'\n'); print(json.dumps({k:v for k,v in row.items() if k!='command'}),flush=True)
    return row

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args()
    out=a.out.resolve(); out.mkdir(exist_ok=False)
    (out/'allowed').write_text('read-only fixture\n'); sentinel=out/'supervisor-secret'; sentinel.write_text('private sentinel\n')
    results=[]
    for fs in ['ext4','xfs','btrfs']:
        disk=out/(fs+'.img')
        with disk.open('wb') as f: f.truncate(512*1024*1024)
        flags={'ext4':['-F','-q'],'xfs':['-f'],'btrfs':['-f']}[fs]
        mk=subprocess.run(['mkfs.'+fs,*flags,str(disk)],capture_output=True)
        (out/(fs+'-mkfs.log')).write_bytes(mk.stdout+mk.stderr)
        if mk.returncode: raise RuntimeError('mkfs failed '+fs)
        for phase in ['write','verify']:
            result=run(out,fs,phase,disk,sentinel); results.append(result)
            if not result['pass']: break
    assert sentinel.read_text()=='private sentinel\n'
    assert (out/'allowed').read_text()=='read-only fixture\n'
    (out/'results.json').write_text(json.dumps({'host':list(os.uname()),'uid':os.getuid(),'results':results},indent=2)+'\n')
if __name__=='__main__': main()
