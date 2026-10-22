#!/usr/bin/env python3
"""Label an owned Fedora root in confined UML, then boot with SELinux enforcing."""
import argparse,hashlib,json,os,selectors,shutil,signal,stat,subprocess,tempfile,time
from pathlib import Path
import probe

INIT='''#!/bin/sh
set -eu
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export TMPDIR=/tmp
busybox mount -t proc proc /proc
busybox mount -t sysfs sysfs /sys
busybox mount -t securityfs securityfs /sys/kernel/security
test "$(busybox cat /sys/kernel/security/lsm)" = capability
busybox mount -t ext4 /dev/ubda /mnt
busybox mkdir -p /mnt/packages /mnt/proc /mnt/sys /mnt/dev
busybox mount -t hostfs -o /packages none /mnt/packages
busybox mount -t proc proc /mnt/proc
busybox mount -t sysfs sysfs /mnt/sys
busybox mount -t devtmpfs devtmpfs /mnt/dev
busybox mkdir -p /tool
busybox mount -t hostfs -o /toolroot none /tool
busybox mount --bind /mnt /tool/work
busybox mount -t proc proc /tool/work/proc
busybox mount -t sysfs sysfs /tool/work/sys
busybox mount -t devtmpfs devtmpfs /tool/work/dev
busybox mount -t hostfs -o /packages none /tool/rpms
busybox mount -t proc proc /tool/proc
busybox mount -t sysfs sysfs /tool/sys
busybox mount -t devtmpfs devtmpfs /tool/dev
busybox mount -t tmpfs -o size=128M tmpfs /tool/tmp
busybox chroot /tool /usr/bin/dnf5 --installroot=/work --releasever=44 --disable-repo='*' --repofrompath=fixture,file:///rpms --setopt=fixture.gpgcheck=0 --setopt=install_weak_deps=False -y install selinux-policy-targeted policycoreutils
busybox umount /tool/tmp
busybox umount /tool/dev
busybox umount /tool/sys
busybox umount /tool/proc
busybox umount /tool/rpms
busybox umount /tool/work/dev
busybox umount /tool/work/sys
busybox umount /tool/work/proc
busybox umount /tool/work
busybox umount /tool
busybox cp /boot-probe /mnt/conglobate-root-boot-probe
busybox chmod 755 /mnt/conglobate-root-boot-probe
printf 'SELINUX=enforcing\nSELINUXTYPE=targeted\n' > /mnt/etc/selinux/config
busybox chroot /mnt /usr/sbin/setfiles -F -e /proc -e /sys -e /dev -e /packages /etc/selinux/targeted/contexts/files/file_contexts /
/bin/getfattr -n security.selinux /mnt/usr/bin/bash /mnt/etc/passwd /mnt/etc/shadow /mnt/usr/lib/systemd/systemd
busybox umount /mnt/packages
busybox umount /mnt/proc
busybox umount /mnt/sys
busybox umount /mnt/dev
busybox sync
busybox umount /mnt
echo UML_SELINUX_LABELING_PASS
busybox poweroff -f
'''
BOOT='''#!/bin/bash
set -euo pipefail
trap 'echo "UML_SELINUX_BOOT_FAILED_LINE_$LINENO" >/dev/console' ERR
test "$(cat /proc/1/comm)" = systemd
test ! -e /etc/initrd-release
test "$(cat /sys/fs/selinux/enforce)" = 1
test "$(getenforce)" = Enforcing
test -e /sys/fs/selinux/policy
test -f /etc/selinux/targeted/contexts/files/file_contexts
source /etc/os-release
test "$ID" = fedora
ls -Zd /usr/bin/bash /etc/passwd /etc/shadow /usr/lib/systemd/systemd
id -Z
rpm -q selinux-policy-targeted policycoreutils
printf 'UML_FEDORA_SELINUX_ENFORCING_BOOT_PASS\n' >/dev/console
systemctl --no-block poweroff
'''

def bounded(cmd,logpath,deadline,env=None):
    start=time.monotonic();error=None;size=0
    p=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,env=env,start_new_session=True)
    try:
        with logpath.open('wb') as log,selectors.DefaultSelector() as sel:
            sel.register(p.stdout,selectors.EVENT_READ)
            while sel.get_map():
                if time.monotonic()-start>deadline:raise TimeoutError('deadline')
                for key,_ in sel.select(.1):
                    chunk=os.read(key.fd,65536)
                    if not chunk:sel.unregister(key.fileobj);continue
                    log.write(chunk);log.flush();size+=len(chunk)
                    if size>8*1024*1024:raise RuntimeError('log bound')
        p.wait(timeout=5)
    except Exception as e:error=repr(e)
    finally:
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        p.wait(timeout=5);p.stdin.close();p.stdout.close()
    return {'command':cmd,'returncode':p.returncode,'seconds':time.monotonic()-start,'error':error}

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--out',type=Path,required=True);a=ap.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
    source=Path('/tmp/conglobate-fedora-root-boot-v2');rpms=Path('/var/tmp/conglobate-uml-selinux-rpms')
    inventory={str(p):hashlib.file_digest(p.open('rb'),'sha256').hexdigest() for p in [source/'rootfs.raw',source/'kernel',source/'initramfs',*rpms.glob('*.rpm')]}
    tool=Path('/tmp/conglobate-rebuilt-package-seed-v2/root')
    tool_entries=[]
    for f in sorted(tool.rglob('*')):
        entry={'path':str(f.relative_to(tool)),'mode':f.lstat().st_mode}
        if f.is_symlink():entry['target']=os.readlink(f)
        elif f.is_file():
            with f.open('rb') as stream:entry['sha256']=hashlib.file_digest(stream,'sha256').hexdigest()
        tool_entries.append(entry)
    (out/'tool-inputs.json').write_text(json.dumps(tool_entries,indent=2)+'\n')
    (out/'inputs.json').write_text(json.dumps(inventory,indent=2)+'\n')
    disk=out/'rootfs.raw';subprocess.run(['cp','--reflink=auto','--sparse=always',str(source/'rootfs.raw'),str(disk)],check=True)
    initrd=out/'initrd';initrd.write_bytes(probe.archive(INIT,[('boot-probe',stat.S_IFREG|0o755,BOOT.encode())]))
    (out/'init').write_text(INIT);(out/'boot-probe').write_text(BOOT)
    runtime=Path(tempfile.mkdtemp(prefix='conglobate-uml-selinux-',dir='/dev/shm'))
    cmd=json.loads(Path('/var/tmp/conglobate-uml-validation-v4/ext4-write.json').read_text())['command']
    for i,v in enumerate(cmd):
        if v.endswith('ext4-write.cpio'):cmd[i]=str(initrd)
        if v.endswith('ext4.img'):cmd[i]=str(disk)
        if v.startswith('/dev/shm/conglobate-uml-confined-'):cmd[i]=str(runtime)
        if v=='mem=256M':cmd[i]='mem=768M'
    pos=cmd.index('/linux',cmd.index('--chdir'))
    cmd[pos:pos]=['--ro-bind',str(rpms),'/packages','--ro-bind','/tmp/conglobate-rebuilt-package-seed-v2/root','/toolroot']
    result=bounded(cmd,out/'build.log',300)
    text=(out/'build.log').read_text(errors='replace');result['pass']=result['returncode']==0 and result['error'] is None and 'UML_SELINUX_LABELING_PASS' in text.splitlines()
    (out/'build.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
    shutil.rmtree(runtime)
    if not result['pass']:return
    digest=hashlib.file_digest(disk.open('rb'),'sha256').hexdigest()
    cmd=json.loads((source/'command.json').read_text())
    for i,v in enumerate(cmd):
        if v.startswith('file='):cmd[i]=f'file={disk},format=raw,if=none,id=owned,snapshot=on'
        if 'selinux=0' in v:cmd[i]=v.replace('selinux=0','selinux=1 enforcing=1')
    result=bounded(cmd,out/'boot.log',120,dict(os.environ,TMPDIR=str(out)))
    lines=(out/'boot.log').read_text(errors='replace').splitlines()
    result['pass']=result['returncode']==0 and result['error'] is None and lines.count('UML_FEDORA_SELINUX_ENFORCING_BOOT_PASS')==1 and not any('UML_SELINUX_BOOT_FAILED' in l for l in lines)
    result['artifact_unchanged']=digest==hashlib.file_digest(disk.open('rb'),'sha256').hexdigest();result['rootfs_sha256']=digest
    (out/'boot.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
if __name__=='__main__':main()
