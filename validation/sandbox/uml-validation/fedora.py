#!/usr/bin/env python3
"""Fedora separate-root bootstrap, package derivation and mkinitfs under UML."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
previous=Path('/var/tmp/conglobate-uml-network-public-v1');outer=out/'outer';inner=out/'inner'
shutil.copytree(previous/'outer',outer,symlinks=True);shutil.copytree(previous/'inner',inner,symlinks=True)
tool=Path('/tmp/conglobate-rebuilt-package-seed-v2/root')
subprocess.run(['tar','-cf',str(inner/'tool.tar'),'-C',str(tool),'.'],check=True)
ca=Path('/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem');shutil.copy2(ca,inner/'ca.pem')
key=Path('/etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-44-primary');shutil.copy2(key,inner/'fedora-key')
meta={'ca_sha256':hashlib.sha256(ca.read_bytes()).hexdigest(),'tool_tar_sha256':hashlib.sha256((inner/'tool.tar').read_bytes()).hexdigest(),'key_sha256':hashlib.sha256(key.read_bytes()).hexdigest()}
for name in ['mkfs.ext4','tar']:
 binary=Path(shutil.which(name));dest=inner/'usr/bin'/name;shutil.copy2(binary,dest)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
  dest=inner/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64','virtio_blk'],text=True).splitlines():
 if line.startswith('insmod '):
  f=Path(line.split()[1]);d=outer/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(outer),'7.2.10-200.fc44.x86_64'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
init='#!/bin/bash\nset -euo pipefail\nexport PATH=/usr/bin:/usr/sbin:/bin:/sbin\nexport TMPDIR=/tmp\nmount -t proc proc /proc\nmount -t sysfs sysfs /sys\nmount -t devtmpfs devtmpfs /dev\n(while /usr/bin/busybox sleep 30; do echo UML_FEDORA_PROGRESS; /usr/bin/busybox ps; done) &\nfor pair in fd:/proc/self/fd stdin:/proc/self/fd/0 stdout:/proc/self/fd/1 stderr:/proc/self/fd/2; do\n name="${pair%%:*}"; target="${pair#*:}"\n test -L "/dev/$name" || /usr/bin/busybox ln -s "$target" "/dev/$name"\ndone\nmodprobe ipv6\nip link set lo up\nip addr add 10.45.0.2/24 dev eth0\nip link set eth0 up\nip route add default via 10.45.0.1\nmkdir -p /tool /mnt /tmp\n/usr/bin/busybox tar -xpf /tool.tar -C /tool\nprintf \'nameserver 10.0.2.3\\n\' >/tool/etc/resolv.conf\ncp /fedora-key /tool/fedora-key\nmkdir -p /tool/etc/pki/ca-trust/extracted/pem\ncp /ca.pem /tool/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem\nmkfs.ext4 -F -q /dev/ubda\nmount -t ext4 /dev/ubda /mnt\nmkdir -p /tool/target /mnt/dev /mnt/proc /mnt/sys /mnt/etc\ncp /tool/etc/resolv.conf /mnt/etc/resolv.conf\nmkdir -p /mnt/etc/dracut.conf.d\nprintf \'compress="zstd -T1 -3"\\n\' > /mnt/etc/dracut.conf.d/validation.conf\nmount --bind /mnt /tool/target\nmount -t proc proc /tool/proc\nmount -t sysfs sysfs /tool/sys\nmount -t devtmpfs devtmpfs /tool/dev\nmount -t proc proc /tool/target/proc\nmount -t sysfs sysfs /tool/target/sys\nmount -t devtmpfs devtmpfs /tool/target/dev\n/usr/bin/busybox chroot /tool /usr/bin/dnf5 --installroot=/target --releasever=44 --disable-repo=\'*\' --repofrompath=fixture,https://dl.fedoraproject.org/pub/fedora/linux/releases/44/Everything/x86_64/os/ --setopt=fixture.gpgcheck=1 --setopt=fixture.gpgkey=file:///fedora-key --setopt=install_weak_deps=False -y install fedora-release bash coreutils systemd passwd dnf5 kernel-core kernel-modules dracut acl attr libcap\nprintf \'UML_FEDORA_BOOTSTRAP_PASS\\n\'\numount /tool/target/dev /tool/target/sys /tool/target/proc /tool/target\nmount -t proc proc /mnt/proc\nmount -t sysfs sysfs /mnt/sys\nmount -t devtmpfs devtmpfs /mnt/dev\n/usr/bin/busybox chroot /mnt /bin/bash -eu -c \'\n echo derived > /fedora-derived\n chown 1234:2345 /fedora-derived\n setfattr -n user.validation -v persisted /fedora-derived\n setfacl -m u:3456:r-- /fedora-derived\n k=$(ls /lib/modules)\n dracut --force --no-hostonly --add-drivers "virtio_pci virtio_blk" /boot/validation-initrd "$k"\n cp /lib/modules/"$k"/vmlinuz /boot/validation-kernel\n rpm -qa\n\'\ncp /usr/bin/busybox /mnt/bin/validation-busybox\ncat >/mnt/sbin/validation-init <<\'BOOT\'\n#!/bin/bash\nset -eu\ntest "$$" = 1\ntest "$(cat /fedora-derived)" = derived\ntest "$(stat -c \'%u:%g\' /fedora-derived)" = 1234:2345\ntest -f /etc/fedora-release\ngetfattr -n user.validation /fedora-derived\ngetfacl -n /fedora-derived\nuname -a\necho UML_FEDORA_NATIVE_ROOT_BOOT_PASS\nexec -a busybox /bin/validation-busybox poweroff -f\nBOOT\nchmod 755 /mnt/sbin/validation-init\necho UML_FEDORA_DERIVE_FIRMWARE_PASS\numount /mnt/dev /mnt/sys /mnt/proc\n/usr/bin/busybox sync\numount /mnt\nmount -t ext4 -o ro /dev/ubda /mnt\ntest "$(cat /mnt/fedora-derived)" = derived\ntest "$(stat -c \'%u:%g\' /mnt/fedora-derived)" = 1234:2345\numount /mnt\necho UML_FEDORA_OUTPUT_VERIFIED\n/usr/bin/busybox poweroff -f\n'
(inner/'init').write_text(init);(inner/'init').chmod(0o755)
def archive(directory,dest):
 paths=[str(f.relative_to(directory)) for f in sorted(directory.rglob('*'))]
 with dest.open('wb') as f:
  subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=directory,stdout=f,stderr=subprocess.DEVNULL,check=True)
  f.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
archive(inner,outer/'uml.cpio')
launch=(outer/'launch').read_text().replace('--dev-bind /dev/net/tun /dev/net/tun','--dev-bind /dev/net/tun /dev/net/tun --dev-bind /dev/vda /disk').replace('mem=512M','mem=2048M').replace('umid=probe','umid=probe ubd0=/disk')
(outer/'launch').write_text(launch)
prefix=(outer/'init').read_text().split('mkdir /work/uml-runtime')[0]
prefix+='''
modprobe virtio_blk
chown 1000:1000 /dev/vda
mkdir /work/uml-runtime
chown 1000:1000 /work/uml-runtime
chmod 700 /work/uml-runtime
ip netns exec command /launch enabled
echo UML_FEDORA_WORKLOAD_PASS
'''
(outer/'init').write_text(prefix)
archive(outer,out/'initramfs')
# RAM-backed output only. The host never mounts this filesystem.
disk=Path('/dev/shm')/('conglobate-fedora-'+out.name+'.raw')
with disk.open('xb') as f:f.truncate(3*1024*1024*1024)
cmd=json.loads((previous/'command.json').read_text());cmd[cmd.index('-initrd')+1]=str(out/'initramfs');cmd[cmd.index('-m')+1]='6144'
cmd+=['-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
runtime_inputs={}
for f in [out/'initramfs',Path(cmd[cmd.index('-kernel')+1])]:
 with f.open('rb') as stream:runtime_inputs[str(f)]=hashlib.file_digest(stream,'sha256').hexdigest()
(out/'runtime-inputs.json').write_text(json.dumps(runtime_inputs,indent=2)+'\n')
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');(out/'tool.json').write_text(json.dumps(meta,indent=2)+'\n')
start=time.monotonic()
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=1200);code=r.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='1200s timeout'
raw=(out/'results.txt').read_text(errors='replace')
clean=re.sub(r'\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]', '', raw)
lines=clean.splitlines();markers=['UML_FEDORA_BOOTSTRAP_PASS','UML_FEDORA_DERIVE_FIRMWARE_PASS','UML_FEDORA_OUTPUT_VERIFIED','UML_FEDORA_WORKLOAD_PASS','GUEST_EXIT 0']
result={'seconds':time.monotonic()-start,'returncode':code,'error':error,'output':str(disk),'markers':{m:lines.count(m) for m in markers}};result['pass']=code==0 and error is None and all(n==1 for n in result['markers'].values())
with disk.open('rb') as f:result['output_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
