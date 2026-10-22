#!/usr/bin/env python3
"""Boot an owned restricted-build root via conglobate import and corium; no host mounts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--raw-boot', type=Path, required=True)
parser.add_argument('--rpms', type=Path, required=True)
parser.add_argument('--corium', type=Path, required=True)
parser.add_argument('--importer', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--release', default='7.2.11-200.fc44.x86_64')
a = parser.parse_args()
out = a.out.resolve(); out.mkdir()
def digest(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()
def run(cmd, **kwargs):
    return subprocess.run([str(x) for x in cmd], check=True, timeout=120, **kwargs)
raw = a.raw_boot/'rootfs.raw'
expected = json.loads((a.raw_boot/'artifacts.json').read_text())
assert digest(raw) == expected['rootfs_raw_sha256']
assert digest(a.raw_boot/'kernel') == expected['firmware']['kernel']
inputs = {'raw': {'path': str(raw.resolve()), 'sha256': digest(raw)},
          'kernel': {'path': str((a.raw_boot/'kernel').resolve()), 'sha256': digest(a.raw_boot/'kernel')},
          'corium': {'path':str(a.corium.resolve()), 'sha256':digest(a.corium)},
          'importer': {'path':str(a.importer.resolve()), 'sha256':digest(a.importer)}, 'rpms': []}
module_root = out/'modules'; module_root.mkdir()
for package in ('kernel-core', 'kernel-modules-core'):
    rpm = a.rpms/f'{package}-{a.release}.rpm'
    inputs['rpms'].append({'path':str(rpm.resolve()),'sha256':digest(rpm)})
    with (out/f'{package}-extract.log').open('wb') as log:
        producer = subprocess.Popen(['rpm2cpio', str(rpm.resolve())], stdout=subprocess.PIPE, stderr=log)
        try:
            run(['cpio', '-idm', '--quiet', '--no-absolute-filenames', './lib/modules/*'],
                cwd=module_root, stdin=producer.stdout, stdout=log, stderr=log)
        finally:
            producer.stdout.close()
        assert producer.wait(timeout=10) == 0
with (out/'depmod.log').open('wb') as log:
    run(['depmod', '-b', module_root, a.release], stdout=log, stderr=log)
stage = out/'initrd-root'; stage.mkdir()
shutil.copy2(a.corium, stage/'init')
modules=set()
for name in ('virtio_pci','virtio_blk','dm_mod','dm_zero','dm_snapshot','dm_verity','ext4'):
    result=run(['modprobe','-d',module_root,'--set-version',a.release,'--show-depends',name],capture_output=True,text=True)
    for line in result.stdout.splitlines():
        if line.startswith('insmod '): modules.add(Path(line.split()[1]).resolve())
for source in sorted(modules):
    relative=source.relative_to(module_root)
    target=stage/'usr'/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(source,target)
inputs['modules']=[{'path':str(p.relative_to(module_root)),'sha256':digest(p)} for p in sorted(modules)]
(out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
entries=['.']+[str(p.relative_to(stage)) for p in sorted(stage.rglob('*'))]
with (out/'initramfs').open('xb') as image, (out/'cpio.log').open('wb') as log:
    run(['cpio','--null','-o','--format=newc','--quiet'],cwd=stage,input=('\0'.join(entries)+'\0').encode(),stdout=image,stderr=log)
result=subprocess.run([str(a.importer.resolve()),str(raw.resolve()),str(out/'scute')],capture_output=True,text=True,timeout=120)
(out/'import.log').write_text(result.stdout+result.stderr)
result.check_returncode()
root=result.stdout.strip(); assert len(root)==64 and bytes.fromhex(root)
sector=2048; parts=[]
for name, identifier in [('data',root[:32]),('hash',root[32:])]:
    path=out/'scute'/name; size=(path.stat().st_size+511)//512
    parts.append((path,sector,size,str(uuid.UUID(bytes_le=bytes.fromhex(identifier)))))
    sector=((sector+size+2047)//2048)*2048
disk=out/'carapace.gpt'
with disk.open('xb') as f: f.truncate((sector+2048)*512)
table='label: gpt\nsector-size: 512\n'+''.join(f'start={start},size={size},uuid={identifier}\n' for _,start,size,identifier in parts)
(out/'partition-table.txt').write_text(table)
with (out/'sfdisk.log').open('wb') as log:
    run(['sfdisk',disk],input=table.encode(),stdout=log,stderr=log)
    run(['sfdisk','--verify',disk],stdout=log,stderr=log)
with disk.open('r+b') as dest:
    for source,start,_,_ in parts:
        dest.seek(start*512)
        with source.open('rb') as f: shutil.copyfileobj(f,dest,1024*1024)
identity={'root_hash':root,'disk_sha256':digest(disk),'initramfs_sha256':digest(out/'initramfs')}
(out/'artifacts.json').write_text(json.dumps(identity,indent=2)+'\n')
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','2048','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str((a.raw_boot/'kernel').resolve()),'-initrd',str(out/'initramfs'),'-drive',f'file={disk},format=raw,if=none,id=owned,readonly=on','-device','virtio-blk-pci,drive=owned','-append',f'console=ttyS0 corium={root} corium.log=info selinux=0 panic=-1 systemd.unit=multi-user.target']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as log:
    subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=150,check=True)
text=(out/'results.txt').read_text()
assert text.splitlines().count('FEDORA_ROOT_SWITCH_SYSTEMD_PASS '+a.release)==1
assert 'ROOT_BOOT_PROBE_FAILED' not in text and 'message="boot failed"' not in text
assert 'phase="pivoted"' in text
assert digest(disk)==identity['disk_sha256'] and digest(raw)==expected['rootfs_raw_sha256']
print('FEDORA_CONGLOBATE_IMPORT_CORIUM_SYSTEMD_PASS')
