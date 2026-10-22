#!/usr/bin/env python3
"""Compose a previously packaged chain-acl guest with owned boot-base inputs."""
import argparse, hashlib, json, shutil, subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--template',type=Path,required=True)
p.add_argument('--base',type=Path,required=True)
p.add_argument('--helper',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args(); out=a.out.resolve(); out.mkdir()
def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()
base=json.loads((a.base/'artifacts.json').read_text()); disk=(a.base/'carapace.gpt').resolve()
assert digest(disk)==base['disk_sha256']
root=out/'guest-root'; shutil.copytree(a.template/'root',root,symlinks=True)
fixture=Path(__file__).resolve().parent
for name,target in [('init','init'),('worker','chain-worker'),('command','command')]:
    shutil.copy2(fixture/name,root/target); (root/target).chmod(0o755)
shutil.copy2(fixture.parent/'chain-command/launch',root/'command-launch')
shutil.copy2(a.helper,root/'derive-root')
(root/'base-root').write_text(base['root_hash']+'\n')
shutil.copy2(a.template/'vmlinuz',out/'vmlinuz')
inputs={'base_disk_sha256':base['disk_sha256'],'base_root':base['root_hash'],'files':{},'symlinks':{}}
for path in sorted(root.rglob('*'))+[out/'vmlinuz']:
    name=str(path.relative_to(out))
    if path.is_symlink(): inputs['symlinks'][name]=str(path.readlink())
    elif path.is_file(): inputs['files'][name]=digest(path)
(out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
paths=[str(path.relative_to(root)) for path in sorted(root.rglob('*'))]
with (out/'initramfs').open('xb') as image, (out/'cpio.log').open('wb') as log:
    subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=root,stdout=image,stderr=log,check=True,timeout=60)
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','4096','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str(out/'vmlinuz'),'-initrd',str(out/'initramfs'),'-drive',f'file={disk},format=raw,if=none,id=base,readonly=on','-device','virtio-blk-pci,drive=base','-device','virtio-serial-pci','-chardev',f'file,id=export,path={out / "layer.bin"}','-device','virtserialport,chardev=export,name=conglobate.layer','-append','console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as log:
    subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=150)
text=(out/'results.txt').read_text()
for marker in ('RESTRICTED_FEDORA_DERIVATION_PASS','DERIVED_BOOT_LAYER_SEALED','DERIVED_BOOT_SERIAL_EXPORT_PASS','DERIVED_BOOT_EXPORT_AND_CLEANUP_PASS','GUEST_EXIT 0'):
    assert text.splitlines().count(marker)==1,marker
assert digest(disk)==base['disk_sha256']
with (out/'layer.bin').open('rb') as source:
    header=source.readline(4096); assert header.endswith(b'\n')
    entries=json.loads(header)
    assert [entry['name'] for entry in entries]==['data','hash','root']
    for entry in entries:
        assert type(entry['bytes']) is int and 0<entry['bytes']<=128*1024*1024
        remaining=entry['bytes']; h=hashlib.sha256()
        with (out/entry['name']).open('xb') as target:
            while remaining:
                chunk=source.read(min(remaining,1048576)); assert chunk
                remaining-=len(chunk); h.update(chunk); target.write(chunk)
        assert h.hexdigest()==entry['sha256']
    assert not source.read(1)
(out/'export.json').write_text(json.dumps(entries,indent=2)+'\n')
print('RESTRICTED_DERIVED_BOOT_ARTIFACT_PASS')
