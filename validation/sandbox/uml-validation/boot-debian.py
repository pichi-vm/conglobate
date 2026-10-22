#!/usr/bin/env python3
"""Extract firmware through confined debugfs, then boot the owned Debian output."""
import argparse,hashlib,json,os,re,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--build',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
result=json.loads((a.build/'result.json').read_text());assert result['pass'];disk=Path(result['output'])
def digest(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert digest(disk)==result['output_sha256']
versions=set(re.findall(r'vmlinuz-([0-9][a-zA-Z0-9.+_-]+)',(a.build/'results.txt').read_text()))
assert len(versions)==1,versions
version=versions.pop()
for source,name in [('/boot/vmlinuz-'+version,'kernel'),('/boot/initrd.img-'+version,'initramfs')]:
 cmd=['bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv','--setenv','PATH','/usr/sbin:/usr/bin','--ro-bind','/usr','/usr','--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(disk),'/image','--bind',str(out),'/output','--chdir','/','debugfs','-R',f'dump {source} /output/{name}','/image']
 r=subprocess.run(cmd,capture_output=True,timeout=30);(out/(name+'-extract.log')).write_bytes(r.stdout+r.stderr);r.check_returncode();assert 0<(out/name).stat().st_size<64*1024*1024
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','1024','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str(out/'kernel'),'-initrd',str(out/'initramfs'),'-drive',f'file={disk},format=raw,if=none,id=root,snapshot=on','-device','virtio-blk-pci,drive=root','-append','console=ttyS0 root=/dev/vda rootfstype=ext4 rw init=/sbin/validation-init panic=1']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');start=time.monotonic()
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=90,env=dict(os.environ,TMPDIR=str(out)));code=r.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='90s timeout'
lines=(out/'results.txt').read_text(errors='replace').splitlines()
r={'returncode':code,'error':error,'seconds':time.monotonic()-start,'marker_count':lines.count('UML_DEBIAN_NATIVE_ROOT_BOOT_PASS'),'root_unchanged':digest(disk)==result['output_sha256'],'kernel_sha256':digest(out/'kernel'),'initramfs_sha256':digest(out/'initramfs')};r['no_kernel_panic']=not any('Kernel panic' in l for l in lines);r['powered_off']=any('reboot: Power down' in l for l in lines);r['pass']=r['no_kernel_panic'] and r['powered_off'] and code==0 and error is None and r['marker_count']==1 and r['root_unchanged']
(out/'result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
