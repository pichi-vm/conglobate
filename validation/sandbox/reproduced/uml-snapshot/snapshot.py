#!/usr/bin/env python3
"""Compose actual authenticated snapshot -> confined UML -> sealed scute fixture."""
import argparse,hashlib,json,re,shutil,stat,subprocess
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
base=Path('/tmp/conglobate-corium-root-v3');template=Path('/tmp/conglobate-corium-derived-template-v2')
root=out/'guest-root';shutil.copytree(template/'root',root,symlinks=True)
fixture=Path(__file__).resolve().parents[1]/'corium-derived'
def digest(f):
 with f.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
identity=json.loads((base/'artifacts.json').read_text());assert digest(base/'carapace.gpt')==identity['disk_sha256']
for name in ['bwrap','setpriv','grep','rmdir']:
 src=Path(shutil.which(name));shutil.copy2(src,root/'usr/bin'/name)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(src)],text=True)):
  dest=root/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
shutil.copy2(probe.BASE/'linux.uml',root/'linux.uml')
shutil.copy2('/var/tmp/conglobate-uml-filter.bpf',root/'uml-filter.bpf')
shutil.copy2(Path(__file__).resolve().parents[1]/'publication/helper/target/debug/derive-uml',root/'derive-root')
(root/'base-root').write_text(identity['root_hash']+'\n')
command=(fixture/'command').read_text();begin=command.index('test "$(cat /conglobate-root-identity)')
command='#!/bin/bash\nset -euo pipefail\n'+command[begin:]
inner='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
busybox mount -t sysfs sysfs /sys
busybox mount -t securityfs securityfs /sys/kernel/security
test "$(busybox cat /sys/kernel/security/lsm)" = capability
busybox mount -t ext4 /dev/ubda /mnt
busybox cp /command /mnt/command
busybox chroot /mnt /bin/bash /command
busybox rm /mnt/command
busybox sync
busybox umount /mnt
echo UML_SNAPSHOT_UNMOUNTED
busybox poweroff -f
'''
(root/'uml.cpio').write_bytes(probe.archive(inner,[('command',stat.S_IFREG|0o755,command.encode())]))
launch='''#!/bin/bash
set -euo pipefail
export PATH=/usr/bin:/usr/sbin
cg=/sys/fs/cgroup/uml-step
mkdir "$cg"
echo 805306368 > "$cg/memory.max"
echo 0 > "$cg/memory.swap.max"
echo 48 > "$cg/pids.max"
echo 1 > "$cg/memory.oom.group"
mkdir /work/uml-runtime
chown 1000:1000 /work/uml-runtime
chmod 700 /work/uml-runtime
set +e
/bin/bash /uml-child "$1" &
child=$!
wait "$child"
result=$?
set -e
echo 1 > "$cg/cgroup.kill"
for ((i=0;i<500;i++)); do
 if grep -q 'populated 0' "$cg/cgroup.events"; then break; fi
 sleep .01
done
grep -q 'populated 0' "$cg/cgroup.events"
cat "$cg/memory.peak"
rmdir "$cg"
rm -rf /work/uml-runtime
test "$result" = 0
echo UML_SNAPSHOT_CGROUP_EMPTY
'''
child='''#!/bin/bash
set -euo pipefail
echo $$ > /sys/fs/cgroup/uml-step/cgroup.procs
exec 9</uml-filter.bpf
exec /usr/bin/setpriv --reuid=1000 --regid=1000 --clear-groups \\
 /usr/bin/bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \\
 --seccomp 9 --clearenv --setenv PATH /bin --setenv TMPDIR /runtime \\
 --setenv HOME /runtime --setenv LC_ALL C --proc /proc --dev /dev --dir /lib64 \\
 --ro-bind /lib64/libc.so.6 /lib64/libc.so.6 \\
 --ro-bind /lib64/ld-linux-x86-64.so.2 /lib64/ld-linux-x86-64.so.2 \\
 --ro-bind /linux.uml /linux --ro-bind /uml.cpio /initrd \\
 --dev-bind "$1" /disk --bind /work/uml-runtime /runtime --chdir / \\
 /linux mem=256M con=null con0=fd:0,fd:1 ssl=null lsm=capability panic=1 rdinit=/init \\
 initrd=/initrd uml_dir=/runtime umid=probe ubd0=/disk
'''
init=(fixture/'init').read_text().replace('fuse loop','loop').replace('chmod 666 /dev/fuse','mkdir -p /sys/fs/cgroup\nmount -t cgroup2 cgroup2 /sys/fs/cgroup\nmkdir /sys/fs/cgroup/supervisor\necho $$ > /sys/fs/cgroup/supervisor/cgroup.procs\necho "+memory +pids" > /sys/fs/cgroup/cgroup.subtree_control')
for name,content in [('init',init),('uml-launch',launch),('uml-child',child)]:
 (root/name).write_text(content);(root/name).chmod(0o755)
shutil.copy2(template/'vmlinuz',out/'vmlinuz')
inputs={'base_disk_sha256':identity['disk_sha256'],'base_root':identity['root_hash'],'files':{},'symlinks':{}}
for f in sorted(root.rglob('*'))+[out/'vmlinuz']:
 name=str(f.relative_to(out))
 if f.is_symlink():inputs['symlinks'][name]=str(f.readlink())
 elif f.is_file():inputs['files'][name]=digest(f)
(out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
paths=[str(f.relative_to(root)) for f in sorted(root.rglob('*'))]
with (out/'initramfs').open('wb') as target,(out/'cpio.log').open('wb') as log:
 subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=root,stdout=target,stderr=log,check=True,timeout=60)
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','4096','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str(out/'vmlinuz'),'-initrd',str(out/'initramfs'),'-drive',f'file={base / "carapace.gpt"},format=raw,if=none,id=base,readonly=on','-device','virtio-blk-pci,drive=base','-device','virtio-serial-pci','-chardev',f'file,id=export,path={out / "layer.bin"}','-device','virtserialport,chardev=export,name=conglobate.layer','-append','console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as log:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=150)
text=(out/'results.txt').read_text();required=['RESTRICTED_FEDORA_DERIVATION_PASS','UML_SNAPSHOT_UNMOUNTED','UML_SNAPSHOT_CGROUP_EMPTY','DERIVED_BOOT_LAYER_SEALED','DERIVED_BOOT_SERIAL_EXPORT_PASS','DERIVED_BOOT_EXPORT_AND_CLEANUP_PASS','GUEST_EXIT 0']
result={'returncode':r.returncode,'markers':{m:text.splitlines().count(m) for m in required},'base_unchanged':digest(base/'carapace.gpt')==identity['disk_sha256']}
result['pass']=r.returncode==0 and all(n==1 for n in result['markers'].values()) and result['base_unchanged']
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
if not result['pass']:raise SystemExit(1)
with (out/'layer.bin').open('rb') as source:
 entries=json.loads(source.readline(4096));assert [e['name'] for e in entries]==['data','hash','root']
 for e in entries:
  assert type(e['bytes']) is int and 0<e['bytes']<=128*1024*1024
  payload=source.read(e['bytes']);assert len(payload)==e['bytes'] and hashlib.sha256(payload).hexdigest()==e['sha256']
  (out/e['name']).write_bytes(payload)
 assert not source.read(1)
(out/'export.json').write_text(json.dumps(entries,indent=2)+'\n')
