#!/usr/bin/python3
"""Bounded ACL semantics comparison using disposable files and numeric IDs."""
import errno
import json
import os
import pathlib
import stat
import struct
import subprocess
import sys

mode, base_arg, report_arg = sys.argv[1:]
base = pathlib.Path(base_arg)
report = pathlib.Path(report_arg)


def acl(path, default=False):
    name = "system.posix_acl_default" if default else "system.posix_acl_access"
    data = os.getxattr(path, name)
    assert struct.unpack("<I", data[:4]) == (2,)
    return list(struct.iter_unpack("<HHI", data[4:]))


def as_user(uid, operation, expect_success, gid=None):
    pid = os.fork()
    if pid == 0:
        try:
            os.setgroups([])
            os.setgid(uid if gid is None else gid)
            os.setuid(uid)
            try:
                operation()
            except OSError as error:
                assert not expect_success and error.errno in (errno.EACCES, errno.EPERM), error
            else:
                assert expect_success, "operation unexpectedly permitted"
        except BaseException as error:
            print(f"ACL_CHILD_FAILURE {error!r}", flush=True)
            os._exit(1)
        os._exit(0)
    assert os.waitpid(pid, 0)[1] == 0


def create(path, permissions):
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, permissions)
    os.write(fd, b"fixture\n")
    os.close(fd)


def snapshot():
    result = {}
    for path in sorted(base.rglob("*")):
        metadata = path.lstat()
        item = {"mode": stat.S_IMODE(metadata.st_mode),
                "uid": metadata.st_uid, "gid": metadata.st_gid}
        for name in ("system.posix_acl_access", "system.posix_acl_default"):
            try:
                item[name] = os.getxattr(path, name).hex()
            except OSError as error:
                assert error.errno in (errno.ENODATA, errno.EOPNOTSUPP)
        result[str(path.relative_to(base))] = item
    return result


if mode == "verify":
    expected = json.loads(report.read_text())
    actual = snapshot()
    assert actual == expected, (actual, expected)
    print("ACL_KERNEL_REMOUNT_MATCH_PASS", flush=True)
    sys.exit()

base.mkdir()
base.chmod(0o755)
file = base / "grant"
create(file, 0o600)
subprocess.run(["setfacl", "-m", "u:1234:r", str(file)], check=True)
assert stat.S_IMODE(file.stat().st_mode) == 0o640
as_user(1234, file.read_bytes, True)
as_user(1235, file.read_bytes, False)
os.chmod(file, 0o600)
assert [entry for entry in acl(file) if entry[0] == 16] == [(16, 0, 0xffffffff)]
as_user(1234, file.read_bytes, False)
os.chmod(file, 0o640)
as_user(1234, file.read_bytes, True)
print("ACL_SET_CHMOD_ENFORCEMENT_PASS", flush=True)
group = base / "group-grant"
create(group, 0o600)
subprocess.run(["setfacl", "-m", "g:1234:r", str(group)], check=True)
as_user(1236, group.read_bytes, True, gid=1234)
as_user(1236, group.read_bytes, False)
os.chmod(group, 0o600)
as_user(1236, group.read_bytes, False, gid=1234)
print("ACL_NAMED_GROUP_AND_MASK_PASS", flush=True)

parent = base / "inherited"
parent.mkdir(mode=0o755)
subprocess.run(["setfacl", "-m", "d:u::rwx,d:u:1234:rwx,d:g::r-x,d:m::rwx,d:o::---",
                str(parent)], check=True)
previous_umask = os.umask(0o077)
try:
    create(parent / "file", 0o666)
    create(parent / "restricted", 0o600)
    (parent / "directory").mkdir(mode=0o750)
    create(parent / "directory/grandchild", 0o666)
    (base / "no-default").mkdir(mode=0o777)
    create(base / "no-default/file", 0o666)
finally:
    os.umask(previous_umask)
assert stat.S_IMODE((parent / "file").stat().st_mode) == 0o660
assert stat.S_IMODE((parent / "directory").stat().st_mode) == 0o750
assert stat.S_IMODE((parent / "restricted").stat().st_mode) == 0o600
assert stat.S_IMODE((base / "no-default/file").stat().st_mode) == 0o600
assert stat.S_IMODE((base / "no-default").stat().st_mode) == 0o700
assert acl(parent, True) == acl(parent / "directory", True)
as_user(1234, (parent / "file").read_bytes, True)
as_user(1234, (parent / "restricted").read_bytes, False)
as_user(1234, (parent / "directory/grandchild").read_bytes, True)
as_user(1235, (parent / "file").read_bytes, False)
print("ACL_INHERITANCE_UMASK_AND_CREATE_MODE_PASS", flush=True)

# Rename/hardlink retain the access ACL; malformed updates preserve it.
os.link(file, base / "alias")
file.rename(base / "renamed")
assert acl(base / "alias") == acl(base / "renamed")
original = os.getxattr(base / "alias", "system.posix_acl_access")
try:
    os.setxattr(base / "alias", "system.posix_acl_access", b"\x02")
except OSError as error:
    assert error.errno == errno.EINVAL
else:
    raise AssertionError("malformed ACL accepted")
assert os.getxattr(base / "alias", "system.posix_acl_access") == original
subprocess.run(["setfacl", "-b", str(base / "alias")], check=True)
try:
    os.getxattr(base / "alias", "system.posix_acl_access")
except OSError as error:
    assert error.errno == errno.ENODATA
else:
    raise AssertionError("extended ACL not removed")
print("ACL_LINK_REMOVE_AND_MALFORMED_UPDATE_PASS", flush=True)

# Owner can alter ACL despite read-only mode; a different UID cannot.
owner = base / "owner"
create(owner, 0o400)
os.chown(owner, 1234, 2345)
as_user(1235, lambda: os.setxattr(owner, "system.posix_acl_access", original), False)
as_user(1234, lambda: os.setxattr(owner, "system.posix_acl_access", original), True)
try:
    os.setxattr(owner, "system.posix_acl_default",
                os.getxattr(parent, "system.posix_acl_default"))
except OSError as error:
    assert error.errno in (errno.EACCES, errno.ENOTDIR)
else:
    raise AssertionError("default ACL accepted on a regular file")
os.chmod(owner, 0o2640)
as_user(1234, lambda: os.setxattr(owner, "system.posix_acl_access", original), True)
assert not owner.stat().st_mode & stat.S_ISGID
os.chmod(owner, 0o2640)
as_user(1234, lambda: os.setxattr(owner, "system.posix_acl_access", original), True, gid=2345)
assert owner.stat().st_mode & stat.S_ISGID
print("ACL_OWNER_AUTHORIZATION_PASS", flush=True)
# Diagnostic boundary: the current high-level libfuse callback does not expose
# the kernel's extended ACL_KILL_SGID flag. Record capability-aware divergence
# separately from the ordinary UID/GID cases; never count it as a semantic pass.
os.chmod(owner, 0o2640)
code = """import os, pathlib, sys
status = pathlib.Path('/proc/self/status').read_text().splitlines()
caps = int(next(line.split()[1] for line in status if line.startswith('CapEff:')), 16)
assert caps & (1 << 4), 'CAP_FSETID missing from diagnostic caller'
os.setxattr(sys.argv[1], 'system.posix_acl_access', bytes.fromhex(sys.argv[2]))
"""
subprocess.run(["setpriv", "--reuid=1234", "--regid=1234", "--clear-groups",
                "--inh-caps=+fsetid", "--ambient-caps=+fsetid",
                "python3", "-c", code, str(owner), original.hex()], check=True)
if owner.stat().st_mode & stat.S_ISGID:
    print("ACL_NONROOT_FSETID_SGID_PRESERVED", flush=True)
else:
    print("ACL_NONROOT_FSETID_SGID_CLEARED_LIMITATION", flush=True)
os.chmod(owner, 0o2640)
code = """import os, pathlib, sys
status = pathlib.Path('/proc/self/status').read_text().splitlines()
caps = int(next(line.split()[1] for line in status if line.startswith('CapEff:')), 16)
assert not caps & (1 << 4), 'CAP_FSETID was not dropped'
os.setxattr(sys.argv[1], 'system.posix_acl_access', bytes.fromhex(sys.argv[2]))
"""
subprocess.run(["setpriv", "--bounding-set=-fsetid", "python3", "-c",
                code, str(owner), original.hex()], check=True)
if owner.stat().st_mode & stat.S_ISGID:
    print("ACL_ROOT_WITHOUT_FSETID_SGID_PRESERVED_LIMITATION", flush=True)
else:
    print("ACL_ROOT_WITHOUT_FSETID_SGID_CLEARED", flush=True)
# Restore a common final state for the independent on-disk/remount comparison.
os.chmod(owner, 0o2640)
report.write_text(json.dumps(snapshot(), sort_keys=True) + "\n")
print("ACL_COMMON_SEMANTICS_PASS", flush=True)
