#!/usr/bin/env python3
"""Fresh-step persistence and firmware workload, UML versus native VM baseline."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--build',type=Path,required=True);p.add_argument('--distro',choices=['alpine','debian','fedora','nixos'],required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
original=json.loads((a.build/'result.json').read_text());assert original['pass'];source=Path(original['output'])
def digest(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert digest(source)==original['output_sha256']
outer=out/'outer';shutil.copytree('/var/tmp/conglobate-uml-benchmark-v2/outer',outer,symlinks=True)
commands={'alpine':'apk info -v; k=$(ls /lib/modules); mkinitfs -F "base virtio ext4" "$k"','debian':'dpkg-query -W; update-initramfs -u -k all','fedora':'sed -i \"s@/bin/validation-busybox poweroff -f@exec -a busybox /bin/validation-busybox poweroff -f@\" /sbin/validation-init; test \"$(cat <(echo process-substitution))\" = process-substitution; rpm -qa; k=$(ls /lib/modules); dracut --force --no-hostonly --drivers "virtio_pci virtio_blk ext4" --filesystems ext4 /boot/validation-initrd "$k"'}
commands['nixos']='export PATH="$1/sw/bin:/bin" PAGER=cat NIX_PAGER=cat; nix-store --query --requisites "$1" > /closure-list; test -s /closure-list; cp -L "$1/kernel" /boot/cross-kernel; cp -L "$1/initrd" /boot/cross-initrd; setfattr -n user.validation -v persisted /nixos-derived; setfacl -m u:3456:r-- /nixos-derived; getfattr -n user.validation /nixos-derived; getfacl -n /nixos-derived'

script='''#!/bin/sh
set -eu
export PATH=/bin:/usr/bin:/sbin:/usr/sbin
busybox mount -t proc proc /proc 2>/dev/null || true
busybox mount -t sysfs sysfs /sys 2>/dev/null || true
busybox mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
for pair in fd:/proc/self/fd stdin:/proc/self/fd/0 stdout:/proc/self/fd/1 stderr:/proc/self/fd/2; do
 name="${pair%%:*}"; target="${pair#*:}"
 test -L "/dev/$name" || busybox ln -s "$target" "/dev/$name"
done
busybox mkdir -p /mnt
busybox mount -t ext4 @DEVICE@ /mnt
busybox mkdir -p /mnt/proc /mnt/sys /mnt/dev
busybox mount -t proc proc /mnt/proc
busybox mount -t sysfs sysfs /mnt/sys
busybox mount -t devtmpfs devtmpfs /mnt/dev
test "$(busybox cat /mnt/@DISTRO@-derived)" = derived
test "$(busybox stat -c '%u:%g' /mnt/@DISTRO@-derived)" = 1234:2345
/bin/getfattr -n user.validation /mnt/@DISTRO@-derived | busybox grep 'user.validation="persisted"'
/bin/getfacl -n /mnt/@DISTRO@-derived | busybox grep 'user:3456:r--'
echo FIRMWARE_START
busybox cat /proc/uptime
busybox chroot /mnt /bin/sh -eu -c '@COMMAND@'
echo FIRMWARE_DONE
busybox cat /proc/uptime
echo new-step >/mnt/cross-step
busybox umount /mnt/dev /mnt/sys /mnt/proc
busybox sync
busybox umount /mnt
busybox mount -t ext4 -o ro @DEVICE@ /mnt
test "$(busybox cat /mnt/cross-step)" = new-step
busybox umount /mnt
echo CROSS_STEP_PASS
busybox poweroff -f
'''.replace('@DISTRO@',a.distro).replace('@COMMAND@',commands[a.distro])
if a.distro=='nixos':
 script='\n'.join(l for l in script.split('\n') if not l.startswith('/bin/getfattr') and not l.startswith('/bin/getfacl'))
 old="busybox chroot /mnt /bin/sh -eu -c '"+commands['nixos']+"'"
 new='system="$(busybox cat /mnt/boot/validation-init)"\nsystem="${system%/init}"\n'+"busybox chroot /mnt \"$system/sw/bin/bash\" -eu -c '"+commands['nixos']+"' bash \"$system\""
 assert old in script;script=script.replace(old,new)
# Native baseline needs the same metadata inspection executables as UML.
for tool in ['getfattr','getfacl']:
 binary=Path(shutil.which(tool));shutil.copy2(binary,outer/'usr/bin'/tool)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
  d=outer/lib.lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,d)
(outer/'uml.cpio').write_bytes(probe.archive(script.replace('@DEVICE@','/dev/ubda')))
(outer/'workload').write_text(script.replace('@DEVICE@','/dev/vda'))
# More memory for firmware tools; all workers remain bounded by the outer VM.
(outer/'launch').write_text((outer/'launch').read_text().replace('mem=512M','mem=1536M'))
paths=[str(f.relative_to(outer)) for f in sorted(outer.rglob('*'))]
with (out/'initramfs').open('wb') as f:
 subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=outer,stdout=f,stderr=subprocess.DEVNULL,check=True)
 f.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
results=[]
for mode in ['native','uml']:
 disk=Path('/dev/shm')/f'conglobate-cross-step-{out.name}-{mode}.raw';assert not disk.exists();subprocess.run(['cp','--sparse=always',str(source),str(disk)],check=True)
 cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','4096','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel','/tmp/conglobate-network-policy-v4/vmlinuz','-initrd',str(out/'initramfs'),'-append',f'console=ttyS0 rdinit=/init selinux=0 panic=1 quiet bench_native={int(mode=="native")}','-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
 start=time.monotonic()
 with (out/(mode+'.log')).open('wb') as log:
  try:r=subprocess.run(['/usr/bin/time','-v','-o',str(out/(mode+'-time.txt')),*cmd],stdout=log,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL,timeout=600);code=r.returncode;error=None
  except subprocess.TimeoutExpired:code=None;error='600s timeout'
 lines=(out/(mode+'.log')).read_text(errors='replace').splitlines();duration=None
 if 'FIRMWARE_START' in lines and 'FIRMWARE_DONE' in lines:
  duration=float(lines[lines.index('FIRMWARE_DONE')+1].split()[0])-float(lines[lines.index('FIRMWARE_START')+1].split()[0])
 record={'mode':mode,'seconds':time.monotonic()-start,'firmware_guest_seconds':duration,'code':code,'error':error,'source_unchanged':digest(source)==original['output_sha256'],'output':str(disk),'output_sha256':digest(disk),'pass':code==0 and error is None and lines.count('CROSS_STEP_PASS')==1 and not any('Kernel panic' in l for l in lines)}
 record['pass'] &= record['source_unchanged'];results.append(record);print(json.dumps(record),flush=True)
(out/'result.json').write_text(json.dumps({'distro':a.distro,'cases':results,'pass':all(r['pass'] for r in results)},indent=2)+'\n')
