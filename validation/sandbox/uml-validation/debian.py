#!/usr/bin/env python3
"""Debian separate-root bootstrap, package derivation and mkinitfs under UML."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
previous=Path('/var/tmp/conglobate-uml-network-public-v1');outer=out/'outer';inner=out/'inner'
shutil.copytree(previous/'outer',outer,symlinks=True);shutil.copytree(previous/'inner',inner,symlinks=True)
inputs=Path('/var/tmp/conglobate-uml-debian-inputs');meta=json.loads((inputs/'packages.json').read_text())
shutil.copytree(inputs/'tool/usr/share/debootstrap',inner/'usr/share/debootstrap',symlinks=True,dirs_exist_ok=True)
shutil.copytree(inputs/'tool/usr/share/keyrings',inner/'usr/share/keyrings',symlinks=True,dirs_exist_ok=True)
shutil.copy2(inputs/'tool/usr/sbin/debootstrap',inner/'usr/bin/debootstrap')
for name in ['ls','uniq','comm','env','mktemp','perl','awk','mkfs.ext4','tar','wget','gpgv','dpkg-deb','dpkg','grep','sed','cut','head','tail','wc','sort','tr','diff','expr','readlink','basename','dirname','which','ar','gzip','gunzip','xz','unxz','zstd','sha256sum','sha512sum','md5sum','chroot','find','rm','mv','rmdir','touch','id','uname','cat','cp','ln','chmod','chown','mkdir','sleep']:
 binary=Path(shutil.which(name));dest=inner/'usr/bin'/name;shutil.copy2(binary,dest)
 dependencies=subprocess.run(['ldd',str(binary)],capture_output=True,text=True).stdout
 for lib in re.findall(r'(/[^\s()]+)',dependencies):
  dest=inner/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
(inner/'usr/bin/sh').symlink_to('bash')
for folder in ['/usr/share/perl5','/usr/lib64/perl5']:
 if Path(folder).exists():shutil.copytree(folder,inner/folder.lstrip('/'),symlinks=True,dirs_exist_ok=True)
shutil.copytree('/usr/share/dpkg',inner/'usr/share/dpkg',symlinks=True,dirs_exist_ok=True)
for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64','virtio_blk'],text=True).splitlines():
 if line.startswith('insmod '):
  f=Path(line.split()[1]);d=outer/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(outer),'7.2.10-200.fc44.x86_64'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
init='#!/bin/bash\nset -euo pipefail\nexport PATH=/usr/bin:/usr/sbin:/bin:/sbin\nexport TMPDIR=/tmp\ntrap \'status=$?; tail -80 /mnt/debootstrap/debootstrap.log || true; /usr/bin/busybox sync; exit $status\' ERR\nmount -t proc proc /proc\nmount -t sysfs sysfs /sys\nmount -t devtmpfs devtmpfs /dev\nmodprobe ipv6\nip link set lo up\nip addr add 10.45.0.2/24 dev eth0\nip link set eth0 up\nip route add default via 10.45.0.1\nmkdir -p /mnt /tmp\nprintf \'nameserver 10.0.2.3\\n\' >/etc/resolv.conf\nmkfs.ext4 -F -q /dev/ubda\nmount -t ext4 /dev/ubda /mnt\ndebootstrap --arch=amd64 --variant=minbase --keyring=/usr/share/keyrings/debian-archive-keyring.pgp trixie /mnt http://deb.debian.org/debian\necho UML_DEBIAN_BOOTSTRAP_PASS\nmount -t proc proc /mnt/proc\nmount -t sysfs sysfs /mnt/sys\nmount -t devtmpfs devtmpfs /mnt/dev\ncp /etc/resolv.conf /mnt/etc/resolv.conf\nprintf \'#!/bin/sh\\nexit 101\\n\' >/mnt/usr/sbin/policy-rc.d\nchmod 755 /mnt/usr/sbin/policy-rc.d\nexport DEBIAN_FRONTEND=noninteractive\nchroot /mnt /usr/bin/apt-get update\nchroot /mnt /usr/bin/apt-get -y --no-install-recommends install linux-image-amd64 initramfs-tools acl attr libcap2-bin\nchroot /mnt /usr/bin/apt-get -y upgrade\nchroot /mnt /bin/sh -eu -c \'\n groupadd -g 2345 buildgroup\n useradd -u 1234 -g 2345 builduser\n echo derived > /debian-derived\n chown 1234:2345 /debian-derived\n setfattr -n user.validation -v persisted /debian-derived\n setfacl -m u:3456:r-- /debian-derived\n update-initramfs -u -k all\n dpkg-query -W\n\'\ncat >/mnt/sbin/validation-init <<\'BOOT\'\n#!/bin/sh\nset -eu\nexport PATH=/usr/sbin:/usr/bin:/sbin:/bin\ntest "$$" = 1\ntest "$(cat /debian-derived)" = derived\ntest "$(stat -c \'%u:%g\' /debian-derived)" = 1234:2345\ntest -f /etc/debian_version\ngetfattr -n user.validation /debian-derived\ngetfacl -n /debian-derived\nuname -a\necho UML_DEBIAN_NATIVE_ROOT_BOOT_PASS\n/bin/busybox poweroff -f\nBOOT\ncp /usr/bin/busybox /mnt/bin/busybox\nchmod 755 /mnt/sbin/validation-init\nls -l /mnt/boot\necho UML_DEBIAN_DERIVE_FIRMWARE_PASS\numount /mnt/dev /mnt/sys /mnt/proc\n/usr/bin/busybox sync\numount /mnt\nmount -t ext4 -o ro /dev/ubda /mnt\ntest "$(cat /mnt/debian-derived)" = derived\ntest "$(stat -c \'%u:%g\' /mnt/debian-derived)" = 1234:2345\numount /mnt\necho UML_DEBIAN_OUTPUT_VERIFIED\n/usr/bin/busybox poweroff -f\n'
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
echo UML_DEBIAN_WORKLOAD_PASS
'''
(outer/'init').write_text(prefix)
archive(outer,out/'initramfs')
# RAM-backed output only. The host never mounts this filesystem.
disk=Path('/dev/shm')/('conglobate-debian-'+out.name+'.raw')
with disk.open('xb') as f:f.truncate(2*1024*1024*1024)
cmd=json.loads((previous/'command.json').read_text());cmd[cmd.index('-initrd')+1]=str(out/'initramfs');cmd[cmd.index('-m')+1]='6144'
cmd+=['-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
runtime_inputs={}
for f in [out/'initramfs',Path(cmd[cmd.index('-kernel')+1])]:
 with f.open('rb') as stream:runtime_inputs[str(f)]=hashlib.file_digest(stream,'sha256').hexdigest()
(out/'runtime-inputs.json').write_text(json.dumps(runtime_inputs,indent=2)+'\n')
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');(out/'tool.json').write_text(json.dumps(meta,indent=2)+'\n')
start=time.monotonic()
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=600);code=r.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='600s timeout'
lines=(out/'results.txt').read_text(errors='replace').splitlines();markers=['UML_DEBIAN_BOOTSTRAP_PASS','UML_DEBIAN_DERIVE_FIRMWARE_PASS','UML_DEBIAN_OUTPUT_VERIFIED','UML_DEBIAN_WORKLOAD_PASS','GUEST_EXIT 0']
result={'seconds':time.monotonic()-start,'returncode':code,'error':error,'output':str(disk),'markers':{m:lines.count(m) for m in markers}};result['pass']=code==0 and error is None and all(n==1 for n in result['markers'].values())
with disk.open('rb') as f:result['output_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
