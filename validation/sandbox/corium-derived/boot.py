#!/usr/bin/env python3
"""Boot exactly the base plus restricted-command derived scute in an owned guest."""
import argparse,hashlib,json,shutil,subprocess,uuid
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('base','derived','raw-boot','out'): p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args(); out=a.out.resolve(); out.mkdir()
def digest(p):
    with p.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()
base=json.loads((a.base/'artifacts.json').read_text()); source=a.base/'carapace.gpt'
assert digest(source)==base['disk_sha256']
entries=json.loads((a.derived/'export.json').read_text())
for entry in entries:
    file=a.derived/entry['name']; assert file.stat().st_size==entry['bytes'] and digest(file)==entry['sha256']
root=(a.derived/'root').read_text(); assert len(bytes.fromhex(root))==32 and root!=base['root_hash']
disk=out/'carapace.gpt'; shutil.copyfile(source,disk)
sector=(disk.stat().st_size+1048575)//1048576*2048
parts=[]
for name,half in [('data',root[:32]),('hash',root[32:])]:
    file=a.derived/name; size=(file.stat().st_size+511)//512
    parts.append((file,sector,size,str(uuid.UUID(bytes_le=bytes.fromhex(half)))))
    sector=(sector+size+2047)//2048*2048
with disk.open('r+b') as f: f.truncate((sector+2048)*512)
table=(a.base/'partition-table.txt').read_text()+''.join(f'start={start},size={size},uuid={identifier}\n' for _,start,size,identifier in parts)
(out/'partition-table.txt').write_text(table)
with (out/'sfdisk.log').open('wb') as log:
    subprocess.run(['sfdisk',str(disk)],input=table.encode(),stdout=log,stderr=log,check=True,timeout=15)
with disk.open('r+b') as dest:
    for file,start,_,_ in parts:
        dest.seek(start*512)
        with file.open('rb') as f: shutil.copyfileobj(f,dest,1048576)
assert digest(a.base/'initramfs')==base['initramfs_sha256']
expected=json.loads((a.raw_boot/'artifacts.json').read_text())
assert digest(a.raw_boot/'kernel')==expected['firmware']['kernel']
identity={'root_hash':root,'base_root':base['root_hash'],'base_disk_sha256':base['disk_sha256'],'disk_sha256':digest(disk),'layer_files':entries}
(out/'artifacts.json').write_text(json.dumps(identity,indent=2)+'\n')
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','2048','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str((a.raw_boot/'kernel').resolve()),'-initrd',str((a.base/'initramfs').resolve()),'-drive',f'file={disk},format=raw,if=none,id=owned,readonly=on','-device','virtio-blk-pci,drive=owned','-append',f'console=ttyS0 corium={root} corium.log=info selinux=0 panic=-1 systemd.unit=multi-user.target']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as log:
    subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=150)
text=(out/'results.txt').read_text()
for marker in ('DERIVED_CORIUM_ROOT_CONTENT_PASS','FEDORA_ROOT_SWITCH_SYSTEMD_PASS 7.2.11-200.fc44.x86_64'):
    assert text.splitlines().count(marker)==1,marker
assert 'ROOT_BOOT_PROBE_FAILED' not in text and 'message="boot failed"' not in text
assert 'phase="pivoted"' in text
assert digest(source)==base['disk_sha256'] and digest(disk)==identity['disk_sha256']
for entry in entries: assert digest(a.derived/entry['name'])==entry['sha256']
print('RESTRICTED_TWO_SCUTE_CORIUM_SYSTEMD_PASS')
