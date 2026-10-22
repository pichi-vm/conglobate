#!/usr/bin/env python3
"""Same bounded workload on ext4, native KVM versus confined UML within KVM."""
import argparse,hashlib,json,re,shutil,stat,subprocess,time
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
outer=out/'outer';shutil.copytree('/var/tmp/conglobate-uml-network-public-v1/outer',outer,symlinks=True)
subprocess.run(['gcc','-O2','-o',str(out/'bench'),str(Path(__file__).with_name('bench.c'))],check=True)
for module in ['virtio_blk','ext4']:
 for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64',module],text=True).splitlines():
  if line.startswith('insmod '):
   f=Path(line.split()[1]);d=outer/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(outer),'7.2.10-200.fc44.x86_64'],check=True,capture_output=True)
script='''#!/bin/sh
set -eu
export PATH=/bin:/usr/bin
busybox mount -t proc proc /proc 2>/dev/null || true
busybox mount -t sysfs sysfs /sys 2>/dev/null || true
busybox mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
echo BENCH_READY
busybox cat /proc/uptime
busybox mkdir -p /mnt
busybox mount -t ext4 @DEVICE@ /mnt
cd /mnt
/bench
busybox sync
cd /
busybox umount /mnt
echo BENCH_FINISHED
busybox cat /proc/uptime
busybox grep -E 'MemTotal|MemAvailable' /proc/meminfo
busybox poweroff -f
'''
(outer/'uml.cpio').write_bytes(probe.archive(script.replace('@DEVICE@','/dev/ubda'),[('bench',stat.S_IFREG|0o755,(out/'bench').read_bytes())]))
shutil.copy2(out/'bench',outer/'bench')
shutil.copy2(probe.BASE/'busybox',outer/'usr/bin/busybox')
(outer/'usr/bin/sh').symlink_to('busybox')
(outer/'workload').write_text(script.replace('@DEVICE@','/dev/vda'));(outer/'workload').chmod(0o755)
launch=(outer/'launch').read_text().replace('--dev-bind /dev/net/tun /dev/net/tun','--dev-bind /dev/vda /disk').replace('umid=probe','umid=probe ubd0=/disk')
(outer/'launch').write_text(launch)
init='''#!/bin/bash
set -euo pipefail
export PATH=/usr/bin:/usr/sbin:/bin:/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
modprobe virtio_blk
modprobe ext4
if grep -q bench_native=1 /proc/cmdline; then exec /workload; fi
mkdir -p /work/uml-runtime
mount -t tmpfs -o size=1G tmpfs /work/uml-runtime
chown 1000:1000 /dev/vda /work/uml-runtime
chmod 700 /work/uml-runtime
/launch disabled
echo BENCH_OUTER_PASS
/usr/bin/busybox poweroff -f
'''
(outer/'init').write_text(init)
paths=[str(f.relative_to(outer)) for f in sorted(outer.rglob('*'))]
with (out/'initramfs').open('wb') as f:
 subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=outer,stdout=f,stderr=subprocess.DEVNULL,check=True)
 f.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
results=[]
for trial in range(3):
 for mode in ['native','uml']:
  disk=Path('/dev/shm')/f'conglobate-benchmark-{out.name}-{mode}-{trial}.raw'
  with disk.open('xb') as f:f.truncate(256*1024*1024)
  subprocess.run(['mkfs.ext4','-F','-q',str(disk)],check=True)
  label=f'{mode}-{trial}'
  cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','2048','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel','/tmp/conglobate-network-policy-v4/vmlinuz','-initrd',str(out/'initramfs'),'-append',f'console=ttyS0 rdinit=/init selinux=0 panic=1 quiet bench_native={int(mode=="native")}','-drive',f'file={disk},format=raw,if=none,id=output','-device','virtio-blk-pci,drive=output']
  start=time.monotonic()
  with (out/(label+'.log')).open('wb') as log:
   try:r=subprocess.run(['/usr/bin/time','-v','-o',str(out/(label+'-time.txt')),*cmd],stdout=log,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL,timeout=60);code=r.returncode;error=None
   except subprocess.TimeoutExpired:code=None;error='60s timeout'
  lines=(out/(label+'.log')).read_text(errors='replace').splitlines()
  values={parts[1]:float(parts[2]) for l in lines if l.startswith('BENCH ') for parts in [l.split()]}
  record={'mode':mode,'trial':trial,'seconds':time.monotonic()-start,'code':code,'error':error,'values':values,'pass':code==0 and error is None and not any('Kernel panic' in l for l in lines) and lines.count('BENCH_PASS')==1 and (mode=='native' or lines.count('BENCH_OUTER_PASS')==1)}
  results.append(record);print(json.dumps(record),flush=True)
  disk.unlink()
(out/'result.json').write_text(json.dumps({'cases':results,'pass':all(r['pass'] for r in results),'input_sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [out/'bench',out/'initramfs',Path('/tmp/conglobate-network-policy-v4/vmlinuz')]}},indent=2)+'\n')
