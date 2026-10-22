#!/usr/bin/env python3
"""Actual snapshot writes followed by ordinary failure/timeout; no publication."""
import argparse,hashlib,json,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
previous=Path('/var/tmp/conglobate-uml-snapshot-v4');base=Path('/tmp/conglobate-corium-root-v3/carapace.gpt')
def digest(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
before=digest(base);results=[]
for case in ['failure','timeout']:
 directory=out/case;directory.mkdir();root=directory/'root';shutil.copytree(previous/'guest-root',root,symlinks=True)
 inner='''#!/bin/sh
set -eu
export PATH=/bin
busybox mount -t proc proc /proc
busybox mount -t ext4 /dev/ubda /mnt
echo disposable-write >/mnt/validation-disposable
busybox sync
echo COMPLETE
'''+('exit 42\n' if case=='failure' else 'busybox sleep 60\n')
 (root/'uml.cpio').write_bytes(probe.archive(inner))
 if case=='timeout':
  launch=(root/'uml-launch').read_text().replace('child=$!','child=$!\n(sleep 2; echo 1 > "$cg/cgroup.kill") &\nwatcher=$!').replace('result=$?','result=$?\nkill "$watcher" 2>/dev/null || true\nwait "$watcher" 2>/dev/null || true')
  (root/'uml-launch').write_text(launch)
 init=(root/'init').read_text();a0=init.index('/derive-root\n');b=init.index('for ((i=',a0)
 init=init[:a0]+'''set +e
/derive-root
code=$?
set -e
test "$code" != 0
test ! -e /work/export
test ! -e /sys/fs/cgroup/uml-step
test ! -e /work/uml-runtime
'''+init[b:]
 init=init[:init.index('ports=0')]+'''echo UML_FAILED_SNAPSHOT_CLEANUP_PASS
sleep .1
'''
 (root/'init').write_text(init)
 paths=[str(f.relative_to(root)) for f in sorted(root.rglob('*'))]
 with (directory/'initramfs').open('wb') as f:subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=root,stdout=f,stderr=subprocess.DEVNULL,check=True)
 cmd=json.loads((previous/'command.json').read_text());cmd[cmd.index('-initrd')+1]=str(directory/'initramfs');cmd[cmd.index('-chardev')+1]=f'file,id=export,path={directory/"layer.bin"}'
 start=time.monotonic()
 with (directory/'results.txt').open('wb') as f:
  try:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL,timeout=30);code=r.returncode;error=None
  except subprocess.TimeoutExpired:code=None;error='30s outer deadline'
 lines=(directory/'results.txt').read_text(errors='replace').splitlines()
 record={'case':case,'seconds':time.monotonic()-start,'code':code,'error':error,'base_unchanged':digest(base)==before,'export_bytes':(directory/'layer.bin').stat().st_size,'cleanup_marker':lines.count('UML_FAILED_SNAPSHOT_CLEANUP_PASS'),'guest_exit':lines.count('GUEST_EXIT 0')}
 record['pass']=code==0 and error is None and record['base_unchanged'] and record['export_bytes']==0 and record['cleanup_marker']==1 and record['guest_exit']==1
 results.append(record);print(json.dumps(record),flush=True)
(out/'result.json').write_text(json.dumps({'cases':results,'pass':all(r['pass'] for r in results)},indent=2)+'\n')
