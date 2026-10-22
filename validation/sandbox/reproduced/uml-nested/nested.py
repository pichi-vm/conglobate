#!/usr/bin/env python3
"""Run the bounded UML metadata fixture inside a disposable Fedora-kernel VM."""
import argparse,gzip,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
inner=probe.INIT.replace('@FS@','ext4').replace('@SENTINEL@','/supervisor-secret').replace('@WORK@',probe.WORK)
verify=probe.INIT.replace('@FS@','ext4').replace('@SENTINEL@','/supervisor-secret').replace('@WORK@',probe.VERIFY).replace('mount -t ext4 /dev/ubda','mount -t ext4 -o ro /dev/ubda')
disk=out/'disk'
with disk.open('wb') as f:f.truncate(64*1024*1024)
subprocess.run(['mkfs.ext4','-F','-q',str(disk)],check=True)
extra=[('linux',stat.S_IFREG|0o755,(probe.BASE/'linux.uml').read_bytes()),('inner.cpio',stat.S_IFREG|0o444,probe.archive(inner)),('verify.cpio',stat.S_IFREG|0o444,probe.archive(verify)),('disk',stat.S_IFREG|0o600,disk.read_bytes())]
for name in ['bwrap','setpriv']:
    binary=Path(shutil.which(name));extra.append(('bin/'+name,stat.S_IFREG|0o755,binary.read_bytes()))
    for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
        extra.append((lib.lstrip('/'),stat.S_IFREG|0o755,Path(lib).read_bytes()))
launch='''#!/bin/sh
set -eu
export PATH=/bin
busybox id
bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL --clearenv \\
 --setenv PATH /bin --setenv TMPDIR /runtime --setenv HOME /runtime --setenv LC_ALL C \\
 --proc /proc --dev /dev --dir /lib64 \\
 --ro-bind /lib64/libc.so.6 /lib64/libc.so.6 \\
 --ro-bind /lib64/ld-linux-x86-64.so.2 /lib64/ld-linux-x86-64.so.2 \\
 --ro-bind /linux /linux --ro-bind "$1" /initrd --ro-bind /allowed /allowed \\
 --bind /disk /disk --bind /runtime /runtime --chdir / \\
 /linux mem=256M con=null con0=fd:0,fd:1 ssl=null lsm=capability panic=1 rdinit=/init \\
 initrd=/initrd uml_dir=/runtime umid=probe ubd0=/disk
'''
extra.append(('launch',stat.S_IFREG|0o755,launch.encode()))
outer='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
busybox mount -t sysfs sysfs /sys
busybox mount -t devtmpfs devtmpfs /dev
busybox mkdir -p /runtime
busybox mount -t tmpfs -o size=2G tmpfs /runtime
busybox chmod 700 /runtime
echo private > /supervisor-secret
echo allowed > /allowed
busybox chmod 444 /allowed
busybox chown 1000:1000 /runtime /disk
busybox cat /proc/uptime
/bin/setpriv --reuid=1000 --regid=1000 --clear-groups /launch /inner.cpio
/bin/setpriv --reuid=1000 --regid=1000 --clear-groups /launch /verify.cpio
test "$(busybox cat /supervisor-secret)" = private
busybox cat /proc/uptime
echo NESTED_UML_PASS
busybox poweroff -f
'''
(out/'outer.init').write_text(outer);(out/'launch').write_text(launch)
(out/'initramfs.gz').write_bytes(gzip.compress(probe.archive(outer,extra),compresslevel=1,mtime=0))
kernel=Path('/boot/vmlinuz-7.2.10-200.fc44.x86_64')
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','4096','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str(kernel),'-initrd',str(out/'initramfs.gz'),'-append','console=ttyS0 rdinit=/init selinux=0 panic=1 quiet']
start=time.monotonic()
with (out/'run.log').open('wb') as log:
    try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=90);error=None;code=r.returncode
    except subprocess.TimeoutExpired as e:error=str(e);code=None
text=(out/'run.log').read_text(errors='replace')
result={'command':cmd,'returncode':code,'error':error,'seconds':time.monotonic()-start,'pass':code==0 and '\nNESTED_UML_PASS\n' in text and text.count('\nCOMPLETE\n')==2,'kernel_sha256':hashlib.sha256(kernel.read_bytes()).hexdigest(),'initramfs_sha256':hashlib.sha256((out/'initramfs.gz').read_bytes()).hexdigest()}
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
