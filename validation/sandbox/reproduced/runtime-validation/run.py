#!/usr/bin/env python3
"""Build a no-NIC/no-shares disposable guest; host operations are unprivileged."""
import argparse,pathlib,shutil,subprocess,re,os,json,hashlib
p=argparse.ArgumentParser();p.add_argument('out',type=pathlib.Path);p.add_argument('--runtimes',default='bwrap crun runc youki libcontainer nsjail systemd nspawn unshare');p.add_argument('--run',action='store_true');p.add_argument('--seed',type=pathlib.Path,default='/var/tmp/conglobate-two-sandboxes-v10');p.add_argument('--downloads',type=pathlib.Path,default='/var/tmp/conglobate-runtime-validation-downloads');p.add_argument('--sources',type=pathlib.Path,default='/var/tmp/conglobate-runtime-comparison-twxvsqho');a=p.parse_args()
here=pathlib.Path(__file__).resolve().parent;out=a.out.resolve();out.mkdir();root=out/'root'
seed=a.seed.resolve()
shutil.copytree(seed/'root',root,symlinks=True)
(root/'tmp').chmod(0o1777)
download=a.downloads.resolve();sysroot=download/'sysroot'
env=os.environ|{'LD_LIBRARY_PATH':str(sysroot/'usr/lib64')}
def copy(src,dst=None):
 src=pathlib.Path(src);target=root/(dst or str(src)).lstrip('/');target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,target)
def binary(src,dst=None):
 copy(src,dst)
 r=subprocess.run(['ldd',str(src)],capture_output=True,text=True,env=env)
 assert 'not found' not in r.stdout,r.stdout
 for name in re.findall(r'(/[^\s()]+)',r.stdout):
  path=pathlib.Path(name)
  if path.is_file():copy(path,'/usr/lib64/'+path.name if str(path).startswith(str(sysroot)) else None)
for tool in ('crun','bwrap','systemd-nspawn','systemd-run','systemctl','nsenter','setpriv'):
 binary(shutil.which(tool))
binary('/usr/bin/dbus-broker');binary('/usr/bin/dbus-broker-launch')
binary('/usr/lib/systemd/systemd')
binary('/usr/lib/systemd/systemd-executor')
binary('/usr/lib/systemd/systemd-shutdown')
for name in ('libbpf.so.1','libnss_systemd.so.2','libnss_files.so.2'):
 if pathlib.Path('/usr/lib64/'+name).exists():binary('/usr/lib64/'+name)
binary(sysroot/'usr/bin/runc','/usr/bin/runc')
binary(download/'youki/youki','/usr/bin/youki')
binary(download/'libcontainer-target/debug/conglobate-libcontainer-probe','/usr/bin/libcontainer-probe')
binary(a.sources/'nsjail/nsjail','/usr/bin/nsjail')
copy(here/'matrix.py','/matrix.py')
(root/'runtimes').write_text(a.runtimes+'\n')
(root/'etc/os-release').write_text('ID=fedora\nNAME=Fedora\nVERSION_ID=44\n')
(root/'etc/passwd').write_text('root:x:0:0:root:/:/bin/bash\nnobody:x:65534:65534:nobody:/:/sbin/nologin\n')
(root/'etc/group').write_text('root:x:0:\nnobody:x:65534:\n')
(root/'etc/machine-id').write_text('cf682345e10943518abc709675342103\n')
(root/'etc/systemd/system').mkdir(parents=True,exist_ok=True)
(root/'empty').mkdir(exist_ok=True)
(root/'usr/share/dbus-1').mkdir(parents=True,exist_ok=True)
(root/'usr/share/dbus-1/system.conf').write_text('<busconfig><type>system</type><user>root</user><listen>unix:path=/run/dbus/system_bus_socket</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
(root/'etc/systemd/system/dbus.socket').write_text('[Unit]\nDefaultDependencies=no\n[Socket]\nListenStream=/run/dbus/system_bus_socket\n')
(root/'etc/systemd/system/dbus.service').write_text('[Unit]\nDefaultDependencies=no\nRequires=dbus.socket\nAfter=dbus.socket\n[Service]\nType=notify\nExecStart=/usr/bin/dbus-broker-launch --scope system --config-file=/usr/share/dbus-1/system.conf\nSockets=dbus.socket\nStandardOutput=tty\nStandardError=inherit\nTTYPath=/dev/console\n')
(root/'etc/systemd/system/runtime-validation.service').write_text('''[Unit]
DefaultDependencies=no
Wants=dbus.socket
After=dbus.socket
[Service]
Type=oneshot
ExecStart=/bin/bash /run-tests
StandardOutput=tty
StandardError=inherit
TTYPath=/dev/console
TimeoutStartSec=600
''')
(root/'init').write_text('''#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
mount --make-rprivate /
export SYSTEMD_UNIT_PATH=/etc/systemd/system
export SYSTEMD_GENERATOR_PATH=/empty
export SYSTEMD_ENVIRONMENT_GENERATOR_PATH=/empty
exec /usr/lib/systemd/systemd --system --unit=runtime-validation.service --log-target=console
''')
(root/'run-tests').write_text('''#!/bin/bash
set -eu
export PATH=/usr/bin:/usr/sbin
trap 'echo "GUEST_EXIT $?"; sleep .1; echo b >/proc/sysrq-trigger' EXIT
mount -t tmpfs tmpfs /work
modprobe fuse
modprobe loop
mount -t fusectl none /sys/fs/fuse/connections
mkdir -p /sys/fs/cgroup/runtime-tests
python3 /matrix.py
''')
(root/'init').chmod(0o755)
shutil.copy2(seed/'vmlinuz',out/'vmlinuz')
inventory={str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in root.rglob('*') if f.is_file() and not f.is_symlink()}
(out/'symlinks.json').write_text(json.dumps({str(f.relative_to(root)):os.readlink(f) for f in root.rglob('*') if f.is_symlink()},indent=2)+'\n')
(out/'inputs.json').write_text(json.dumps(inventory,indent=2)+'\n')
paths=[str(f.relative_to(root)) for f in sorted(root.rglob('*'))]
r=subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=b'\0'.join(s.encode() for s in paths)+b'\0',cwd=root,capture_output=True,check=True)
with (out/'initramfs.gz').open('wb') as f:subprocess.run(['gzip','-1','-n'],input=r.stdout,stdout=f,check=True)
cmd=json.loads((seed/'command.json').read_text());cmd=[v.replace(str(seed),str(out)) for v in cmd];cmd[cmd.index('-m')+1]='6144'
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
if a.run:
 with (out/'results.txt').open('wb') as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=660,check=True)
 text=(out/'results.txt').read_text();assert len(re.findall(r'^GUEST_EXIT 0\r?$',text,re.M))==1,text[-3000:]
 rows=[json.loads(l.split(' ',1)[1]) for l in text.splitlines() if l.startswith('RUNTIME_RESULT ')]
 assert {r['runtime'] for r in rows}==set(a.runtimes.split()),rows
 assert 'RUNTIME_MATRIX_COMPLETE' in text
 (out/'verdict.json').write_text(json.dumps({'results':rows,'production_qualified':False},indent=2)+'\n')
 print(json.dumps(rows,indent=2))
