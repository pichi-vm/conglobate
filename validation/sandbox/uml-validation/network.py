#!/usr/bin/env python3
"""Private VM-only dual-stack UML TAP/NAT/DNS validation. No host NIC/share."""
import argparse,hashlib,json,re,shutil,stat,subprocess
from pathlib import Path
import probe
p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);p.add_argument('--public',action='store_true');a=p.parse_args();out=a.out.resolve();out.mkdir(exist_ok=False)
template=Path('/tmp/conglobate-network-policy-v4');fixture=Path(__file__).resolve().parents[1]/'network-policy'
root=out/'outer';inner=out/'inner';shutil.copytree(template/'root',root,symlinks=True);shutil.copytree(template/'root',inner,symlinks=True)
def put(base,name,text):
 f=base/name;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(text);f.chmod(0o755)
def archive(directory,dest):
 paths=[str(f.relative_to(directory)) for f in sorted(directory.rglob('*'))]
 with dest.open('wb') as target:
  subprocess.run(['cpio','--null','-o','--format=newc','--owner=0:0'],input=('\0'.join(paths)+'\0').encode(),cwd=directory,stdout=target,stderr=subprocess.DEVNULL,check=True)
  target.write(probe.cpio([('dev/console',stat.S_IFCHR|0o600,b'',5,1)]))
for name in ['bwrap','setpriv','grep','rmdir']:
 src=Path(shutil.which(name));shutil.copy2(src,root/'usr/bin'/name)
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(src)],text=True)):
  dest=root/lib.lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(lib,dest)
umlmods=Path('/var/tmp/conglobate-debian-uml-extracted/usr/lib/uml/modules/6.12.101')
modout=inner/'usr/lib/modules/6.12.101';modout.mkdir(parents=True,exist_ok=True)
deps=dict(line.split(':',1) for line in (umlmods/'modules.dep').read_text().splitlines())
def copymod(name):
 target=modout/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(umlmods/name,target)
 for dep in deps[name].split():copymod(dep)
copymod(next(name for name in deps if '/ipv6.ko' in name))
for name in ['modules.builtin','modules.builtin.modinfo']:
 if (umlmods/name).exists():shutil.copy2(umlmods/name,modout/name)
subprocess.run(['depmod','-b',str(inner),'6.12.101'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)

module=Path(subprocess.check_output(['modinfo','-k','7.2.10-200.fc44.x86_64','-n','tun'],text=True).strip());dest=root/str(module).lstrip('/');dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(module,dest)
if a.public:
 for m in ['virtio_pci','virtio_net']:
  for line in subprocess.check_output(['modprobe','--show-depends','--set-version','7.2.10-200.fc44.x86_64',m],text=True).splitlines():
   if line.startswith('insmod '):
    f=Path(line.split()[1]);d=root/str(f).lstrip('/');d.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,d)
subprocess.run(['depmod','-b',str(root),'7.2.10-200.fc44.x86_64'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
shutil.copy2(probe.BASE/'linux.uml',root/'linux.uml');shutil.copy2('/var/tmp/conglobate-uml-filter.bpf',root/'uml-filter.bpf')
client=(fixture/'client.py').read_text();client='\n'.join(line for line in client.splitlines() if not line.startswith('status =') and not line.startswith('assert int(status'))+'\n'
put(inner,'client.py',client)
listener=(fixture/'listener.py').read_text();listener='\n'.join(line for line in listener.splitlines() if not line.startswith('status =') and not line.startswith('assert int(status'))+'\n'
listener=listener.replace('time.sleep(3)','print("UML_LISTENER_READY",flush=True)\ntime.sleep(5)')
put(inner,'listener.py',listener)
put(inner,'resolver','nameserver 172.27.0.2\nsearch build.test\noptions timeout:1 attempts:1\n')
put(inner,'disabled.py', '''import socket
for addr in [('172.27.0.2',8080),('fd42:27::2',8080)]:
 try:socket.create_connection(addr,.3)
 except OSError:pass
 else:raise AssertionError('disabled network reached upstream')
try:socket.getaddrinfo('mirror.build.test',8080)
except socket.gaierror:pass
else:raise AssertionError('disabled DNS resolved')
print('UML_DISABLED_NETWORK_PASS',flush=True)
''')
put(inner,'init','''#!/bin/bash
set -euo pipefail
export PATH=/usr/bin:/usr/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
if [[ "$(cat /proc/cmdline)" == *probe_disabled=1* ]]; then
 python3 /disabled.py
 /usr/bin/busybox poweroff -f
fi
modprobe ipv6
ip link set lo up
ip addr add 10.45.0.2/24 dev eth0
ip -6 addr add fd42:45::2/64 dev eth0 nodad
ip link set eth0 up
ip route add default via 10.45.0.1
ip -6 route add default via fd42:45::1
for style in regular symlink; do
 if test "$style" = symlink; then rm /etc/resolv.conf; ln -s /run/resolv.original /etc/resolv.conf; fi
 mkdir -p /run
 printf 'nameserver 127.0.0.53\noptions timeout:1 attempts:1\n' >/etc/resolv.conf
 python3 /stub.py
 mount --bind /resolver /etc/resolv.conf
 mount -o remount,bind,ro /etc/resolv.conf
 python3 /client.py
 umount /etc/resolv.conf
 python3 -c 'from pathlib import Path; assert "127.0.0.53" in Path("/etc/resolv.conf").read_text()'
 echo UML_RESOLVER_RESTORED_$style
done
python3 /listener.py
echo UML_ENABLED_NETWORK_PASS
/usr/bin/busybox poweroff -f
''')
shutil.copy2(probe.BASE/'busybox',inner/'usr/bin/busybox')
if a.public:
 put(inner,'public.py', '''import socket
from pathlib import Path
Path('/etc/resolv.conf').write_text('nameserver 10.0.2.3\\noptions timeout:2 attempts:2\\n')
addresses=socket.getaddrinfo('deb.debian.org',80,socket.AF_INET,socket.SOCK_STREAM)
assert addresses
with socket.create_connection(addresses[0][4],5) as s:
 s.sendall(b'HEAD /debian/ HTTP/1.0\\r\\nHost: deb.debian.org\\r\\n\\r\\n')
 reply=s.recv(4096)
 assert reply.startswith(b'HTTP/1.') and reply.split()[1] in (b'200',b'301',b'302'),reply
print('UML_PUBLIC_DNS_HTTP_PASS',flush=True)
''')
 f=inner/'init';f.write_text(f.read_text().replace('python3 /listener.py','python3 /public.py\npython3 /listener.py'))
archive(inner,root/'uml.cpio')
baseinit=(fixture/'init').read_text();prefix=baseinit[:baseinit.index("printf 'nameserver")]
prefix=prefix.replace('echo b >/proc/sysrq-trigger', 'sleep .2; echo b >/proc/sysrq-trigger')
prefix=prefix.replace('nft_ct;','nft_ct tun;').replace('ip netns add upstream','chmod 666 /dev/net/tun\nip netns add upstream')
prefix=prefix.replace('ip saddr 10.44.0.0/24','ip saddr 10.45.0.0/24').replace('ip6 saddr fd42:44::/64','ip6 saddr fd42:45::/64')
prefix+='''
ip netns exec command ip tuntap add tap0 mode tap user 1000
ip -n command addr add 10.45.0.1/24 dev tap0
ip -n command -6 addr add fd42:45::1/64 dev tap0 nodad
ip -n command link set tap0 up
ip netns exec command sysctl -w net.ipv4.ip_forward=1
ip netns exec command sysctl -w net.ipv6.conf.all.forwarding=1
ip route add 10.45.0.0/24 via 10.44.0.2
ip -6 route add fd42:45::/64 via fd42:44::2
ip -n upstream route add 10.45.0.0/24 via 172.27.0.1
ip -n upstream -6 route add fd42:45::/64 via fd42:27::1
mkdir /work/uml-runtime
chown 1000:1000 /work/uml-runtime
chmod 700 /work/uml-runtime
ip netns exec command /launch enabled > /work/uml-enabled.log 2>&1 &
runner=$!
ready=0
for ((i=0;i<1200;i++)); do
 if grep -q UML_LISTENER_READY /work/uml-enabled.log; then ready=1; break; fi
 if ! kill -0 "$runner" 2>/dev/null; then set +e; wait "$runner"; echo UML_RUNNER_EXIT_$?; set -e; cat /work/uml-enabled.log; exit 1; fi
 sleep .05
done
test "$ready" = 1
ip netns exec command python3 - <<'CONTROL'
import socket
for addr in [('10.45.0.2',8081),('fd42:45::2',8081)]:
 with socket.create_connection(addr,1):pass
print('UML_LISTENER_POSITIVE_CONTROLS_PASS',flush=True)
CONTROL
ip netns exec upstream python3 - <<'INBOUND'
import socket
for addr in [('10.45.0.2',8081),('fd42:45::2',8081)]:
 try:socket.create_connection(addr,.4)
 except OSError:pass
 else:raise AssertionError('unsolicited inbound accepted')
print('UML_UNSOLICITED_INBOUND_BLOCKED_PASS',flush=True)
INBOUND
set +e
wait "$runner"
result=$?
set -e
cat /work/uml-enabled.log
test "$result" = 0
ip netns exec disabled /launch disabled
nft list ruleset
echo UML_NETWORK_SUITE_PASS
'''
if a.public:
 public_setup='''modprobe virtio_pci
modprobe virtio_net
ip addr add 10.0.2.15/24 dev eth0
ip link set eth0 up
ip route add default via 10.0.2.2
nft add rule inet guard forward iifname vm-step oifname eth0 accept
nft add rule ip nat postrouting oifname eth0 ip saddr 10.45.0.0/24 masquerade
'''
 prefix=prefix.replace('mkdir /work/uml-runtime',public_setup+'mkdir /work/uml-runtime')
put(root,'init',prefix)
put(root,'launch','''#!/bin/bash
set -euo pipefail
args=()
if test "$1" = enabled; then args+=(eth0=tuntap,tap0); else args+=(probe_disabled=1); fi
exec 9</uml-filter.bpf
exec setpriv --reuid=1000 --regid=1000 --clear-groups \\
 bwrap --unshare-all --share-net --die-with-parent --new-session --cap-drop ALL --seccomp 9 \\
 --clearenv --setenv PATH /bin --setenv HOME /runtime --setenv TMPDIR /runtime --setenv LC_ALL C \\
 --proc /proc --dev /dev --dev-bind /dev/net/tun /dev/net/tun --dir /lib64 \\
 --ro-bind /lib64/libc.so.6 /lib64/libc.so.6 --ro-bind /lib64/ld-linux-x86-64.so.2 /lib64/ld-linux-x86-64.so.2 \\
 --ro-bind /linux.uml /linux --ro-bind /uml.cpio /initrd --bind /work/uml-runtime /runtime --chdir / \\
 /linux mem=512M con=null con0=fd:0,fd:1 ssl=null lsm=capability panic=1 rdinit=/init \\
 initrd=/initrd uml_dir=/runtime umid=probe "${args[@]}"
''')
archive(root,out/'initramfs')
inputs={}
for f in sorted(root.rglob('*')):
 if f.is_file() and not f.is_symlink():
  with f.open('rb') as stream:inputs[str(f.relative_to(root))]=hashlib.file_digest(stream,'sha256').hexdigest()
(out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
cmd=['qemu-system-x86_64','-enable-kvm','-cpu','host','-m','4096','-smp','2','-nodefaults','-no-reboot','-display','none','-serial','stdio','-monitor','none','-kernel',str(template/'vmlinuz'),'-initrd',str(out/'initramfs'),'-append','console=ttyS0 rdinit=/init selinux=0 panic=1 quiet']
if a.public:cmd+=['-netdev','user,id=public','-device','virtio-net-pci,netdev=public']
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (out/'results.txt').open('wb') as log:
 try:r=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,timeout=120);error=None;code=r.returncode
 except subprocess.TimeoutExpired:error='120s deadline';code=None
lines=(out/'results.txt').read_text(errors='replace').splitlines()
markers=['UML_NETWORK_SUITE_PASS','UML_ENABLED_NETWORK_PASS','UML_DISABLED_NETWORK_PASS','UML_RESOLVER_RESTORED_regular','UML_RESOLVER_RESTORED_symlink','UML_LISTENER_POSITIVE_CONTROLS_PASS','UML_UNSOLICITED_INBOUND_BLOCKED_PASS','GUEST_EXIT 0']
if a.public:markers.append('UML_PUBLIC_DNS_HTTP_PASS')
result={'returncode':code,'error':error,'markers':{m:lines.count(m) for m in markers}};result['pass']=code==0 and error is None and all(n==1 for n in result['markers'].values())
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
