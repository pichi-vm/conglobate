#!/usr/bin/env python3
"""Alpine separate-root bootstrap, package derivation and mkinitfs under UML."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
previous=Path('/var/tmp/conglobate-uml-network-public-v1');outer=out/'outer';inner=out/'inner'
shutil.copytree(previous/'outer',outer,symlinks=True);shutil.copytree(previous/'inner',inner,symlinks=True)
inputs=Path('/var/tmp/conglobate-uml-alpine-inputs');meta=json.loads((inputs/'inputs.json').read_text());tool=inputs/meta['file'];assert hashlib.sha256(tool.read_bytes()).hexdigest()==meta['sha256']
shutil.copy2(tool,inner/'tool.tar.gz')
for name in ['mkfs.ext4','tar']:
 binary=Path(shutil.which(name));dest=inner/'usr/bin'/name;shutil.copy2(binary,dest)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
  dest=inner/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64','virtio_blk'],text=True).splitlines():
 if line.startswith('insmod '):
  f=Path(line.split()[1]);d=outer/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(outer),'7.2.10-200.fc44.x86_64'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
init='''#!/bin/bash
set -euo pipefail
export PATH=/usr/bin:/usr/sbin:/bin:/sbin
export TMPDIR=/tmp
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
modprobe ipv6
ip link set lo up
ip addr add 10.45.0.2/24 dev eth0
ip link set eth0 up
ip route add default via 10.45.0.1
mkdir -p /tool /mnt /tmp
/usr/bin/busybox tar -xzpf /tool.tar.gz -C /tool
printf 'nameserver 10.0.2.3\n' >/tool/etc/resolv.conf
mkfs.ext4 -F -q /dev/ubda
mount -t ext4 /dev/ubda /mnt
mkdir -p /tool/target /mnt/dev /mnt/proc /mnt/sys /mnt/etc/apk
cp -a /tool/etc/apk/keys /mnt/etc/apk/
cp /tool/etc/apk/repositories /mnt/etc/apk/
cp /tool/etc/resolv.conf /mnt/etc/resolv.conf
mount --bind /mnt /tool/target
mount -t proc proc /tool/proc
mount -t sysfs sysfs /tool/sys
mount -t devtmpfs devtmpfs /tool/dev
mount -t proc proc /tool/target/proc
mount -t sysfs sysfs /tool/target/sys
mount -t devtmpfs devtmpfs /tool/target/dev
/usr/bin/busybox chroot /tool /sbin/apk --root /target --initdb --update-cache add alpine-base linux-virt mkinitfs
echo UML_ALPINE_BOOTSTRAP_PASS
umount /tool/target/dev /tool/target/sys /tool/target/proc
umount /tool/target
mount -t proc proc /mnt/proc
mount -t sysfs sysfs /mnt/sys
mount -t devtmpfs devtmpfs /mnt/dev
/usr/bin/busybox chroot /mnt /sbin/apk --update-cache add acl attr libcap-utils bash
/usr/bin/busybox chroot /mnt /sbin/apk upgrade
/usr/bin/busybox chroot /mnt /bin/sh -eu -c '
 addgroup -g 2345 buildgroup
 adduser -D -u 1234 -G buildgroup builduser
 echo derived > /alpine-derived
 chown 1234:2345 /alpine-derived
 setfattr -n user.validation -v persisted /alpine-derived
 setfacl -m u:3456:r-- /alpine-derived
 k=$(ls /lib/modules)
 mkinitfs -F "base virtio ext4" "$k"
 test -s /boot/vmlinuz-virt
 test -s /boot/initramfs-virt
 apk info -v
'
cat >/mnt/sbin/validation-init <<'BOOT'
#!/bin/sh
set -eu
test "$$" = 1
test "$(cat /alpine-derived)" = derived
test "$(stat -c '%u:%g' /alpine-derived)" = 1234:2345
test -f /etc/alpine-release
getfattr -n user.validation /alpine-derived
getfacl -n /alpine-derived
uname -a
echo UML_ALPINE_NATIVE_ROOT_BOOT_PASS
poweroff -f
BOOT
chmod 755 /mnt/sbin/validation-init
echo UML_ALPINE_DERIVE_FIRMWARE_PASS
umount /mnt/dev /mnt/sys /mnt/proc
/usr/bin/busybox sync
umount /mnt
mount -t ext4 -o ro /dev/ubda /mnt
test "$(cat /mnt/alpine-derived)" = derived
test "$(stat -c '%u:%g' /mnt/alpine-derived)" = 1234:2345
umount /mnt
echo UML_ALPINE_OUTPUT_VERIFIED
/usr/bin/busybox poweroff -f
'''
(inner/'init').write_text(init);(inner/'init').chmod(0o755)
def archive(directory,dest):
 paths=[str(f.relative_to(directory)) for f in sorted(directory.rglob('*'))]
 with dest.open('wb') as f:
  subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=directory,stdout=f,stderr=subprocess.DEVNULL,check=True)
  f.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
archive(inner,outer/'uml.cpio')
launch=(outer/'launch').read_text().replace('--dev-bind /dev/net/tun /dev/net/tun','--dev-bind /dev/net/tun /dev/net/tun --dev-bind /dev/vda /disk').replace('mem=512M','mem=1536M').replace('umid=probe','umid=probe ubd0=/disk')
(outer/'launch').write_text(launch)
prefix=(outer/'init').read_text().split('mkdir /work/uml-runtime')[0]
prefix+='''
modprobe virtio_blk
chown 1000:1000 /dev/vda
mkdir /work/uml-runtime
chown 1000:1000 /work/uml-runtime
chmod 700 /work/uml-runtime
ip netns exec command /launch enabled
echo UML_ALPINE_WORKLOAD_PASS
'''
(outer/'init').write_text(prefix)
archive(outer,out/'initramfs')
# RAM-backed output only. The host never mounts this filesystem.
disk=Path('/dev/shm')/('conglobate-alpine-'+out.name+'.raw')
with disk.open('xb') as f:f.truncate(1024*1024*1024)
cmd=json.loads((previous/'command.json').read_text());cmd[cmd.index('-initrd')+1]=str(out/'initramfs');cmd[cmd.index('-m')+1]='6144'
cmd+=['-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
runtime_inputs={}
for f in [out/'initramfs',Path(cmd[cmd.index('-kernel')+1])]:
 with f.open('rb') as stream:runtime_inputs[str(f)]=hashlib.file_digest(stream,'sha256').hexdigest()
(out/'runtime-inputs.json').write_text(json.dumps(runtime_inputs,indent=2)+'\n')
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');(out/'tool.json').write_text(json.dumps(meta,indent=2)+'\n')
start=time.monotonic()
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=300);code=r.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='300s timeout'
lines=(out/'results.txt').read_text(errors='replace').splitlines();markers=['UML_ALPINE_BOOTSTRAP_PASS','UML_ALPINE_DERIVE_FIRMWARE_PASS','UML_ALPINE_OUTPUT_VERIFIED','UML_ALPINE_WORKLOAD_PASS','GUEST_EXIT 0']
result={'seconds':time.monotonic()-start,'returncode':code,'error':error,'output':str(disk),'markers':{m:lines.count(m) for m in markers}};result['pass']=code==0 and error is None and all(n==1 for n in result['markers'].values())
with disk.open('rb') as f:result['output_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
