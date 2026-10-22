#!/usr/bin/env python3
"""NixOS separate-root installation and sandboxed derivation under UML."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
previous=Path('/var/tmp/conglobate-uml-network-public-v1');outer=out/'outer';inner=out/'inner'
shutil.copytree(previous/'outer',outer,symlinks=True);shutil.copytree(previous/'inner',inner,symlinks=True)
inputs=Path('/var/tmp/conglobate-uml-nix-inputs');meta=json.loads((inputs/'inputs.json').read_text())
for source,name,digest in [(inputs/meta['nix']['file'],'nix.tar.xz',meta['nix']['sha256']),(inputs/'nixexprs.tar.xz','nixpkgs.tar.xz',meta['nixpkgs']['sha256'])]:
 assert hashlib.sha256(source.read_bytes()).hexdigest()==digest;shutil.copy2(source,inner/name)
for name in ['mkfs.ext4','tar','xz','ls','sh','stat','sync','chroot','readlink']:
 binary=Path(shutil.which(name));dest=inner/'usr/bin'/name;shutil.copy2(binary,dest)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.run(['ldd',str(binary)],capture_output=True,text=True).stdout):
  dest=inner/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64','virtio_blk'],text=True).splitlines():
 if line.startswith('insmod '):
  f=Path(line.split()[1]);d=outer/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(outer),'7.2.10-200.fc44.x86_64'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
init='#!/bin/bash\nset -euo pipefail\nexport PATH=/usr/bin:/usr/sbin:/bin:/sbin\nexport TMPDIR=/tmp\nexport PAGER=cat NIX_PAGER=cat TERM=dumb\nmount -t proc proc /proc\nmount -t sysfs sysfs /sys\nmount -t devtmpfs devtmpfs /dev\nmkdir -p /dev/pts\nmount -t devpts devpts /dev/pts\nmodprobe ipv6\nip link set lo up\nip addr add 10.45.0.2/24 dev eth0\nip link set eth0 up\nip route add default via 10.45.0.1\nmkdir -p /work /mnt /tmp /nix /nixpkgs /etc/nix /fallback\nchmod 1777 /tmp\n/usr/bin/busybox --install -s /fallback\nexport PATH="$PATH:/fallback"\nprintf \'root:x:0:0:root:/root:/bin/bash\\n\' >/etc/passwd\nprintf \'root:x:0:\\n\' >/etc/group\nmkdir -p /root\nprintf \'nameserver 10.0.2.3\\n\' >/etc/resolv.conf\nmount -t tmpfs -o size=3G tmpfs /nix\ntar -xJf /nix.tar.xz -C /work\ntar -xJf /nixpkgs.tar.xz -C /nixpkgs --strip-components=1\ncp -a /work/nix-2.35.3-x86_64-linux/store /nix/\nexport PATH=/nix/store/kp71jf0isr8pix0nn2lx2x3hj3vasii5-nix-2.35.3/bin:$PATH\nexport NIX_SSL_CERT_FILE=/nix/store/zdl7dn1gmi1cxdw5a8hw6xsf4cz0rjmg-nss-cacert-3.123/etc/ssl/certs/ca-bundle.crt\nexport NIX_PATH=nixpkgs=/nixpkgs\nexport NIX_CONFIG=\'build-users-group =\nsandbox = true\nmax-jobs = 1\ncores = 1\n\'\nnix-store --load-db < /work/nix-2.35.3-x86_64-linux/.reginfo\nnix --version\nnix-build /compile.nix --no-out-link\nsystem=$(nix-build /nixpkgs/nixos -A system --no-out-link -I nixos-config=/configuration.nix)\necho "$system" >/system-path\ninstaller=$(nix-build /nixpkgs -A nixos-install-tools --no-out-link)\nmkfs.ext4 -F -q /dev/ubda\nmount -t ext4 /dev/ubda /mnt\n"$installer/bin/nixos-install" --root /mnt --system "$system" --no-bootloader --no-root-passwd\nmkdir -p /mnt/boot\ncp -L "$system/kernel" /mnt/boot/validation-kernel\ncp -L "$system/initrd" /mnt/boot/validation-initrd\nprintf \'%s/init\\n\' "$system" >/mnt/boot/validation-init\nprintf \'derived\\n\' >/mnt/nixos-derived\nchown 1234:2345 /mnt/nixos-derived\nprintf \'UML_NIXOS_INSTALL_FIRMWARE_PASS\\n\'\nnix-store --query --requisites "$system" > /closure-list\ncat /closure-list\n/usr/bin/busybox sync\numount /mnt\nmount -t ext4 -o ro /dev/ubda /mnt\ntest "$(cat /mnt/nixos-derived)" = derived\ntest "$(stat -c \'%u:%g\' /mnt/nixos-derived)" = 1234:2345\ntest -s /mnt/boot/validation-kernel\ntest -s /mnt/boot/validation-initrd\numount /mnt\necho UML_NIXOS_OUTPUT_VERIFIED\n/usr/bin/busybox poweroff -f\n'
(inner/'real-init').write_text(init);(inner/'real-init').chmod(0o755)
(inner/'init').write_text('#!/bin/sh\nset -eu\n/usr/bin/busybox mkdir /newroot\n/usr/bin/busybox mount -t tmpfs -o size=6G tmpfs /newroot\nfor p in /*; do\n case "$p" in /newroot) continue;; esac\n /usr/bin/busybox cp -a "$p" /newroot/\ndone\n/usr/bin/busybox mv /newroot/real-init /newroot/init\nexec /usr/bin/busybox switch_root /newroot /init\n');(inner/'init').chmod(0o755)
def archive(directory,dest):
 paths=[str(f.relative_to(directory)) for f in sorted(directory.rglob('*'))]
 with dest.open('wb') as f:
  subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=directory,stdout=f,stderr=subprocess.DEVNULL,check=True)
  f.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
(inner/'configuration.nix').write_text('{ config, pkgs, lib, ... }: {\n imports = [ /nixpkgs/nixos/modules/profiles/minimal.nix ];\n system.stateVersion = "26.05";\n boot.loader.grub.enable = false;\n boot.initrd.systemd.enable = false;\n boot.initrd.availableKernelModules = [ "virtio_pci" "virtio_blk" ];\n boot.kernelParams = [ "console=ttyS0" ];\n fileSystems."/" = { device = "/dev/vda"; fsType = "ext4"; };\n networking.hostName = "uml-validation";\n networking.useDHCP = false;\n environment.systemPackages = [ pkgs.acl pkgs.attr ];\n systemd.services.uml-validation = {\n  wantedBy = [ "multi-user.target" ];\n  after = [ "local-fs.target" ];\n  serviceConfig = { Type = "oneshot"; StandardOutput = "journal+console"; };\n  script = \'\'\n   test "$(cat /nixos-derived)" = derived\n   test "$(stat -c \'%u:%g\' /nixos-derived)" = 1234:2345\n   test -e /etc/NIXOS\n   echo UML_NIXOS_NATIVE_ROOT_BOOT_PASS > /dev/console\n   ${pkgs.systemd}/bin/systemctl --no-block poweroff\n  \'\';\n };\n}\n')
(inner/'compile.nix').write_text('let pkgs = import /nixpkgs {}; in pkgs.runCommand "uml-compile-validation" { nativeBuildInputs = [ pkgs.stdenv.cc ]; } \'\'\n printf \'#include <stdio.h>\\nint main(void) { puts("UML_NIX_COMPILE_PASS"); return 0; }\\n\' > main.c\n cc main.c -o "$out"\n "$out"\n\'\'\n')
archive(inner,outer/'uml.cpio')
launch=(outer/'launch').read_text().replace('--dev-bind /dev/net/tun /dev/net/tun','--dev-bind /dev/net/tun /dev/net/tun --dev-bind /dev/vda /disk').replace('mem=512M','mem=6144M').replace('umid=probe','umid=probe ubd0=/disk')
(outer/'launch').write_text(launch)
prefix=(outer/'init').read_text().split('mkdir /work/uml-runtime')[0]
prefix+='''
modprobe virtio_blk
chown 1000:1000 /dev/vda
mkdir /work/uml-runtime
chown 1000:1000 /work/uml-runtime
chmod 700 /work/uml-runtime
ip netns exec command /launch enabled
echo UML_NIXOS_WORKLOAD_PASS
'''
(outer/'init').write_text(prefix)
archive(outer,out/'initramfs')
# RAM-backed output only. The host never mounts this filesystem.
disk=Path('/dev/shm')/('conglobate-nixos-'+out.name+'.raw')
with disk.open('xb') as f:f.truncate(6*1024*1024*1024)
cmd=json.loads((previous/'command.json').read_text());cmd[cmd.index('-initrd')+1]=str(out/'initramfs');cmd[cmd.index('-m')+1]='10240'
cmd+=['-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
runtime_inputs={}
for f in [out/'initramfs',Path(cmd[cmd.index('-kernel')+1])]:
 with f.open('rb') as stream:runtime_inputs[str(f)]=hashlib.file_digest(stream,'sha256').hexdigest()
(out/'runtime-inputs.json').write_text(json.dumps(runtime_inputs,indent=2)+'\n')
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');(out/'tool.json').write_text(json.dumps(meta,indent=2)+'\n')
start=time.monotonic()
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=900);code=r.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='900s timeout'
lines=(out/'results.txt').read_text(errors='replace').splitlines();markers=['UML_NIX_COMPILE_PASS','UML_NIXOS_INSTALL_FIRMWARE_PASS','UML_NIXOS_OUTPUT_VERIFIED','UML_NIXOS_WORKLOAD_PASS','GUEST_EXIT 0']
result={'seconds':time.monotonic()-start,'returncode':code,'error':error,'output':str(disk),'markers':{m:lines.count(m) for m in markers}};result['pass']=code==0 and error is None and all(n==1 for n in result['markers'].values())
with disk.open('rb') as f:result['output_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
