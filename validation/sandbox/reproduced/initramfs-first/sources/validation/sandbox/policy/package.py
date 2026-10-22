#!/usr/bin/env python3
"""Rerun the retained Fedora/FUSE fixture with the candidate command policy.

Use package-fixture/build.py to assemble --seed from repository scripts and
explicit package/tool inputs. Every packaged input is hashed.
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--seed", required=True, type=pathlib.Path)
parser.add_argument("--out", required=True, type=pathlib.Path)
parser.add_argument("--restricted-bootstrap", action="store_true")
parser.add_argument("--initramfs-rpms", type=pathlib.Path)
parser.add_argument("--protocol-worker-build", type=pathlib.Path,
                    help="optional corrected libfuse/ACL worker pair")
parser.add_argument("--upgrade-rpms", type=pathlib.Path,
                    help="optional output of package-upgrade/build.py")
args = parser.parse_args()
if args.initramfs_rpms is not None and not args.restricted_bootstrap:
    parser.error("--initramfs-rpms requires --restricted-bootstrap")
out = args.out.resolve()
out.mkdir()
root = out / "root"
shutil.copytree(args.seed / "root", root, symlinks=True)
helper = pathlib.Path(__file__).resolve().parents[1] / "entry/helper"
subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                str(helper / "Cargo.toml")], check=True)
shutil.copy2(helper / "target/debug/conglobate-entry-probe", root / "entry-helper")


def replace(file, before, after):
    path = root / file
    text = path.read_text()
    assert text.count(before) == 1, (file, before)
    path.write_text(text.replace(before, after))


if args.protocol_worker_build is not None:
    shutil.copy2(args.protocol_worker_build / "acl-patched", root / "paired-worker")
    shutil.copy2(args.protocol_worker_build / "libfuse3.so.4", root / "usr/lib64/libfuse3.so.4")
    replace("worker", "fuse2fs -f", "/paired-worker -f")
    replace("semantics.py", "status=os.waitpid(pid,0)[1];print('CHILD_STATUS',status,flush=True)",
            "status=os.waitpid(pid,0)[1];print('CHILD_STATUS',status,flush=True);assert status==0")
    replace("semantics.py", "print('ACL_INHERITANCE_PROBE_END',flush=True)",
            """acl_text=subprocess.check_output(['getfacl','-cpn',g],text=True)
assert 'user:1234:rwx' in acl_text and 'mask::rw-' in acl_text,acl_text
print('PAIRED_WORKER_PACKAGE_ACL_ASSERTIONS_PASS',flush=True)
print('ACL_INHERITANCE_PROBE_END',flush=True)""")

if args.restricted_bootstrap:
    bootstrap = pathlib.Path(__file__).resolve().parents[1] / "restricted-bootstrap"
    shutil.copy2(bootstrap / "launch", root / "bootstrap-launch")
    shutil.copy2(bootstrap / "command", root / "bootstrap-command")
    replace("init", "mkdir /work/raw /work/view /work/runtime",
            "mkdir /work/raw /work/view /work/runtime /work/tool\nchown 100000:100000 /work/tool")
    replace("worker", "dnf5 --no-plugins --disable-repo='*' --installroot=\"$p\" --releasever=44 --use-host-config --setopt=install_weak_deps=False --no-gpgchecks -y install /rpms/*.rpm",
            "unshare --mount --pid --net --ipc --uts --fork /bin/bash /bootstrap-launch </dev/null 2>&1 | cat")

if args.initramfs_rpms is not None:
    rpms = sorted(args.initramfs_rpms.glob("*.rpm"))
    if not rpms:
        raise RuntimeError("No initramfs RPMs provided")
    (root / "initramfs-rpms").mkdir()
    for rpm in rpms:
        shutil.copy2(rpm, root / "initramfs-rpms" / rpm.name)
    replace("bootstrap-launch", "for directory in usr etc rpms; do",
            "for directory in usr etc rpms initramfs-rpms; do\n mkdir -p \"$p/$directory\"")
    replace("bootstrap-command", "echo RESTRICTED_INITIAL_DNF_INSTALL_PASS",
            "dnf5 --no-plugins --disable-repo='*' --installroot=/output --releasever=44 --use-host-config --setopt=install_weak_deps=False --no-gpgchecks -y install /initramfs-rpms/*.rpm\necho RESTRICTED_INITIAL_DNF_INSTALL_PASS")
    fixture = pathlib.Path(__file__).resolve().parents[1] / "initramfs/command"
    shutil.copy2(fixture, root / "initramfs-command")
    replace("worker", "echo PACKAGE_INSTALL_PASS",
            "echo PACKAGE_INSTALL_PASS\ncp /initramfs-command \"$p/initramfs-command\"")
    replace("sandbox-test", "chmod 000 /dev/null",
            "/bin/bash /initramfs-command\nchmod 000 /dev/null")
    replace("init", "umount /work/raw",
            "chroot /work/raw sha256sum -c /boot/validation-initramfs.sha256\necho INITRAMFS_KERNEL_REMOUNT_PASS\numount /work/raw")

replace("sandbox-entry", 'exec /entry-helper "$p" /sandbox-test',
        'exec /entry-helper "$p" /sandbox-test --restricted')
replace("worker", 'echo PACKAGE_INSTALL_PASS', '''echo PACKAGE_INSTALL_PASS
# Explicitly stage tools and one package as fixture inputs. The command cannot
# reach the supervisor's package directory or the verified context.
cp /usr/bin/rpm "$p/usr/bin/rpm"
cp -a /usr/lib/rpm "$p/usr/lib/"
cp -a /usr/lib64/. "$p/usr/lib64/"
cp /rpms/attr-*.rpm "$p/transaction-fixture.rpm"''')
replace("sandbox-test", "chmod 000 /dev/null", '''rpm --dbpath /usr/lib/sysimage/rpm --nosignature --nodeps --replacepkgs -U /transaction-fixture.rpm
rpm --dbpath /usr/lib/sysimage/rpm --nodeps -e attr
test ! -e /usr/bin/getfattr
rpm --dbpath /usr/lib/sysimage/rpm --nosignature --nodeps -i /transaction-fixture.rpm
test -x /usr/bin/getfattr
getfattr -n security.selinux /test-owned
echo RESTRICTED_RPM_REINSTALL_REMOVE_INSTALL_PASS
chmod 000 /dev/null''')
if args.upgrade_rpms is not None:
    upgrade = pathlib.Path(__file__).resolve().parents[1] / "package-upgrade"
    for version in ("1", "2"):
        shutil.copy2(args.upgrade_rpms / f"version-{version}.rpm",
                     root / f"upgrade-{version}.rpm")
    shutil.copy2(upgrade / "command", root / "upgrade-command")
    replace("worker", "echo PACKAGE_INSTALL_PASS", """echo PACKAGE_INSTALL_PASS
cp /upgrade-1.rpm /upgrade-2.rpm /upgrade-command "$p/" """)
    replace("sandbox-test", "chmod 000 /dev/null",
            "/bin/bash /upgrade-command\nchmod 000 /dev/null")
    replace("init", "umount /work/raw", """test "$(cat /work/raw/usr/share/conglobate-upgrade/version)" = 2
test "$(cat /work/raw/etc/conglobate-upgrade.conf)" = 'administrator edit'
test "$(cat /work/raw/etc/conglobate-upgrade.conf.rpmnew)" = default-2
test "$(stat -c '%a:%u:%g' /work/raw/var/lib/conglobate-upgrade/state)" = 640:1234:2345
echo UPGRADE_KERNEL_REMOUNT_PASS
umount /work/raw""")
replace("adversarial.py", "assert ctypes.get_errno()==errno.ESRCH,ctypes.get_errno()",
        "assert ctypes.get_errno()==errno.EPERM,ctypes.get_errno()")
replace("adversarial.py", """ assert libc.mount(b'proc',b'/proc-probe',b'proc',0,None)==0,ctypes.get_errno()
 try:
  pid=int(open('/daemon-pid').read())
  assert not os.path.exists('/proc-probe/'+str(pid))
  assert os.readlink('/proc-probe/1/root')=='/'
 finally:assert libc.umount2(b'/proc-probe',0)==0""",
        """ assert libc.mount(b'proc',b'/proc-probe',b'proc',0,None)==-1
 assert ctypes.get_errno()==errno.EPERM,ctypes.get_errno()""")
replace("adversarial.py", "fresh proc mount cannot reveal ancestor PIDs",
        "new proc mount denied by policy")
replace("init", "sync; echo b", "sync; sleep .1; echo b")
shutil.copy2(args.seed / "vmlinuz", out / "vmlinuz")
inventory = {"seed": str(args.seed.resolve()), "files": {}, "symlinks": {}}
for path in sorted(root.rglob("*")) + [out / "vmlinuz"]:
    relative = str(path.relative_to(out))
    if path.is_symlink():
        inventory["symlinks"][relative] = str(path.readlink())
    elif path.is_file():
        with path.open("rb") as stream:
            inventory["files"][relative] = hashlib.file_digest(stream, "sha256").hexdigest()
(out / "inputs.json").write_text(json.dumps(inventory, indent=2) + "\n")
paths = [str(p.relative_to(root)) for p in sorted(root.rglob("*"))]
archive = subprocess.run(["cpio", "--null", "-o", "--format=newc", "--owner=0:0"],
                         input=b"\0".join(p.encode() for p in paths) + b"\0",
                         cwd=root, capture_output=True, check=True)
with (out / "initramfs.gz").open("wb") as stream:
    subprocess.run(["gzip", "-1", "-n"], input=archive.stdout, stdout=stream, check=True)
command = ["qemu-system-x86_64", "-enable-kvm", "-cpu", "host", "-m", "8192" if args.initramfs_rpms else "4096",
           "-smp", "2", "-nodefaults", "-no-reboot", "-display", "none",
           "-serial", "stdio", "-monitor", "none", "-kernel", str(out / "vmlinuz"),
           "-initrd", str(out / "initramfs.gz"), "-append",
           "console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet"]
(out / "command.json").write_text(json.dumps(command, indent=2) + "\n")
with (out / "results.txt").open("wb") as stream:
    subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT,
                   timeout=300 if args.initramfs_rpms else 150, check=True)
result = (out / "results.txt").read_text()
exits = [line for line in result.splitlines() if line.startswith("GUEST_EXIT")]
assert exits == ["GUEST_EXIT 0"], exits
if args.protocol_worker_build is not None:
    assert "PAIRED_WORKER_PACKAGE_ACL_ASSERTIONS_PASS" in result
    assert "CHILD_STATUS 256" not in result
for marker in ("PACKAGE_INSTALL_PASS", "ADVERSARIAL_RESULT PASS", "SANDBOX_PASS",
               "RESTRICTED_RPM_REINSTALL_REMOVE_INSTALL_PASS",
               "DETACHED_DESCENDANT_TERMINATION_PASS", "DAEMON_SURVIVED_PASS",
               "INSTALLED_SHELL_PASS", "VALIDATION_PASS", "GUEST_EXIT 0"):
    assert marker in result, (marker, out / "results.txt")
assert "SECURITY_FAILURE" not in result
if args.initramfs_rpms is not None:
    assert "RESTRICTED_DRACUT_GENERATION_PASS" in result
    assert "INITRAMFS_KERNEL_REMOUNT_PASS" in result
if args.restricted_bootstrap:
    assert "RESTRICTED_INITIAL_DNF_INSTALL_PASS" in result
if args.upgrade_rpms is not None:
    assert "RESTRICTED_VERSION_UPGRADE_CONFIG_SCRIPTLETS_PASS" in result
    assert "UPGRADE_KERNEL_REMOUNT_PASS" in result
print(f"Restricted FUSE workload evidence: {out}/results.txt")
if args.protocol_worker_build is None:
    print("Known ACL failures remain. Base installation precedes restrictions; RPM transactions run inside.")
elif args.restricted_bootstrap:
    print("Paired-worker ACL assertions and restricted initial installation passed.")
else:
    print("Paired-worker ACL assertions passed. Base installation precedes restrictions; RPM transactions run inside.")
