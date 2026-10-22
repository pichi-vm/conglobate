#!/usr/bin/env python3
"""Assemble the Fedora fixture from repo scripts, pinned RPMs and host tools."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--kernel-version", required=True)
parser.add_argument("--rpms", required=True, type=Path)
parser.add_argument("--out", required=True, type=Path)
args = parser.parse_args()
catalog = json.loads((HERE / "rpms.json").read_text())
# Verify every package before creating a fixture. Extra input-directory files
# are not packaged; only the explicit catalog is consumed.
for name, digest in catalog.items():
    actual = hashlib.sha256((args.rpms / name).read_bytes()).hexdigest()
    if actual != digest:
        raise RuntimeError(f"RPM digest mismatch: {name}")
out = args.out.resolve()
out.mkdir()
root = out / "root"
for name in ("usr/bin", "usr/sbin", "usr/lib", "usr/lib64", "etc/dnf",
             "etc/yum.repos.d", "proc", "sys", "dev", "tmp", "work", "rpms"):
    (root / name).mkdir(parents=True, exist_ok=True)
for name, target in (("bin", "usr/bin"), ("sbin", "usr/sbin"),
                     ("lib", "usr/lib"), ("lib64", "usr/lib64")):
    (root / name).symlink_to(target)
copied = set()


def copy(path):
    path = Path(path)
    if path in copied:
        return
    copied.add(path)
    destination = root / str(path).lstrip("/")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, destination)


tools = """bash blockdev cat chcon chmod chown chroot cp debugfs dmsetup dnf5
e2fsck find fuse2fs fusermount3 getcap getenforce getfacl getfattr getpcaps grep
kill ln losetup mkdir mkfs.ext4 mknod modprobe mount mountpoint pivot_root ps
readlink rm rpm setcap setfacl setfattr setfiles setpriv sh sha256sum sleep stat
sync systemd-sysusers timeout touch truncate umount unshare""".split()
for tool in tools:
    path = shutil.which(tool)
    if path is None:
        raise RuntimeError(f"Host tool unavailable: {tool}")
    copy(path)
    result = subprocess.run(["ldd", path], capture_output=True, text=True)
    if "not found" in result.stdout:
        raise RuntimeError(f"Missing runtime library for {tool}")
    for library in re.findall(r"(/[^\s()]+)", result.stdout):
        if Path(library).is_file():
            copy(library)
shutil.copytree("/usr/lib/rpm", root / "usr/lib/rpm", symlinks=True)
# No host repository definitions, credentials or user configuration are copied.
(root / "etc/dnf/dnf.conf").write_text("[main]\ninstall_weak_deps=False\n")
(root / "etc/yum.repos.d/fixture.repo").write_text(
    "[fixture-disabled]\nname=Disabled fixture repository\n"
    "baseurl=file:///fixture-no-repository\nenabled=0\n"
)
(root / "etc/mtab").symlink_to("/proc/mounts")
(root / "etc/passwd").write_text("root:x:0:0:root:/root:/bin/bash\n")
(root / "etc/group").write_text("root:x:0:\n")
for name in catalog:
    shutil.copy2(args.rpms / name, root / "rpms" / name)
for script in (HERE / "scripts").iterdir():
    if script.is_file():
        shutil.copy2(script, root / script.name)
        (root / script.name).chmod(0o755)
modules = Path("/lib/modules") / args.kernel_version
for metadata in modules.glob("modules.*"):
    if metadata.is_file():
        copy(metadata)
for module in ("fuse", "loop", "dm_snapshot"):
    result = subprocess.check_output(
        ["modprobe", "--set-version", args.kernel_version, "--show-depends", module],
        text=True,
    )
    for line in result.splitlines():
        if line.startswith("insmod "):
            copy(line.split()[1])
shutil.copy2(Path("/boot") / f"vmlinuz-{args.kernel_version}", out / "vmlinuz")
manifest = {"kernel_version": args.kernel_version, "files": {}, "symlinks": {}}
for path in sorted(root.rglob("*")) + [out / "vmlinuz"]:
    name = str(path.relative_to(out))
    if path.is_symlink():
        manifest["symlinks"][name] = str(path.readlink())
    elif path.is_file():
        manifest["files"][name] = hashlib.sha256(path.read_bytes()).hexdigest()
manifest["source_files"] = {
    str(p.relative_to(HERE)): hashlib.sha256(p.read_bytes()).hexdigest()
    for p in [Path(__file__).resolve(), HERE / "rpms.json",
              *(p for p in (HERE / "scripts").iterdir() if p.is_file())]
}
(out / "seed-inputs.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Assembled explicit-input fixture: {out}")
