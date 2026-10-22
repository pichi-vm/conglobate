#!/usr/bin/python3
"""Record bounded metadata semantics; compare FUSE with ordinary kernel ext4."""
import array
import ctypes
import hashlib
import errno
import fcntl
import json
import os
from pathlib import Path
import stat
import struct
import sys

action, base_arg, report_arg = sys.argv[1:]
base, report = Path(base_arg), Path(report_arg)
# Linux x86_64 asm-generic ioctl encoding, from linux/fs.h, long == 8.
GETFLAGS, SETFLAGS = 0x80086601, 0x40086602
assert array.array("l").itemsize == 8


def attempt(fn):
    try:
        value = fn()
        return {"ok": value}
    except OSError as error:
        return {"errno": errno.errorcode[error.errno]}


def flags(path, value=None):
    with path.open("rb") as stream:
        data = array.array("l", [value or 0])
        fcntl.ioctl(stream.fileno(), GETFLAGS if value is None else SETFLAGS, data, True)
        return data[0] if value is None else None


if action == "seed":
    for name, bit in (("immutable", 0x10), ("append", 0x20)):
        path = base / ("inherited-" + name)
        path.write_bytes(b"inherited flag fixture\n")
        flags(path, flags(path) | bit)
    sys.exit(0)


def snapshot():
    result = {}
    for path in sorted(base.rglob("*")):
        st = path.lstat()
        item = {"mode": st.st_mode, "uid": st.st_uid, "gid": st.st_gid,
                "atime_ns": st.st_atime_ns, "mtime_ns": st.st_mtime_ns}
        # Only compare deliberately fixed timestamps; creation times differ.
        if not path.name.startswith("time-"):
            item.pop("atime_ns")
            item.pop("mtime_ns")
        if stat.S_ISREG(st.st_mode):
            if path.name.startswith("io-"):
                item["size"] = st.st_size
                item["blocks"] = st.st_blocks
                item["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            item["flags"] = attempt(lambda: flags(path))
            item["xattrs"] = {
                name: os.getxattr(path, name).hex()
                for name in sorted(os.listxattr(path))
            }
        result[path.name] = item
    return result


if action == "verify":
    for name, bit in (("immutable", 0x10), ("append", 0x20)):
        assert flags(base.parent / ("inherited-" + name)) & bit
    print("INHERITED_FLAGS_KERNEL_REMOUNT_PASS", flush=True)
    expected = json.loads(report.read_text())["disk"]
    actual = snapshot()
    # FUSE-visible flags can differ from persisted flags, so report differences
    # rather than calling a semantic mismatch a pass.
    if actual == expected:
        print("METADATA_KERNEL_REMOUNT_MATCH_PASS", flush=True)
    else:
        print("METADATA_KERNEL_REMOUNT_DIFFERENCE " +
              json.dumps({"before": expected, "after": actual}, sort_keys=True), flush=True)
    sys.exit()
if action == "compare":
    reference = json.loads(base.read_text())
    actual = json.loads(report.read_text())
    for section in reference:
        for name in sorted(reference[section].keys() | actual[section].keys()):
            left, right = reference[section].get(name), actual[section].get(name)
            if left != right:
                print("METADATA_REFERENCE_DIFFERENCE " + json.dumps(
                    {"section": section, "name": name, "kernel": left, "fuse": right},
                    sort_keys=True), flush=True)
    sys.exit()

assert action == "create"
base.mkdir()
results = {}
for name, bit in (("immutable", 0x10), ("append", 0x20)):
    path = base.parent / ("inherited-" + name)
    before = flags(path)
    assert before & bit
    results["inherited-" + name + "-clear"] = attempt(lambda: flags(path, before & ~bit))
    if results["inherited-" + name + "-clear"] == {"ok": None}:
        flags(path, before)
    assert flags(path) == before
for name, seconds in (("time-normal", 1700000000), ("time-before-epoch", -1),
                      ("time-after-2038", 2200000000)):
    path = base / name
    path.write_bytes(b"timestamp fixture\n")
    wanted = (seconds * 10**9 + 123456789, seconds * 10**9 + 987654321)
    results[name] = attempt(lambda: os.utime(path, ns=wanted))
    st = path.stat()
    results[name]["exact"] = (st.st_atime_ns, st.st_mtime_ns) == wanted
link = base / "time-symlink"
link.symlink_to("time-normal")
results["time-symlink"] = attempt(
    lambda: os.utime(link, ns=(1700000000123456789, 1700000000987654321),
                    follow_symlinks=False))

for name, value in (("user.fixture", b"fixture"),
                    ("security.selinux", b"other_policy_u:object_r:fixture_t:s0\0"),
                    ("trusted.fixture", b"fixture")):
    path = base / name
    path.write_bytes(b"xattr fixture\n")
    results[name] = attempt(lambda: os.setxattr(path, name, value))
    if results[name] == {"ok": None}:
        assert os.getxattr(path, name) == value

# VFS_CAP_REVISION_2 | EFFECTIVE; CAP_NET_BIND_SERVICE permitted, no inheritable
# bits. Metadata only: never execute a capability-bearing fixture file.
capability = struct.pack("<IIIII", 0x02000001, 1 << 10, 0, 0, 0)
for operation in ("persist", "write", "chown", "chmod"):
    path = base / ("cap-" + operation)
    path.write_bytes(b"non-executable capability metadata fixture\n")
    os.chmod(path, 0o600)
    results["cap-" + operation + "-set"] = attempt(
        lambda: os.setxattr(path, "security.capability", capability))
    if results["cap-" + operation + "-set"] == {"ok": None}:
        results["cap-" + operation + "-before"] = attempt(
            lambda: os.getxattr(path, "security.capability").hex())
        if operation == "write":
            results["cap-write-operation"] = attempt(lambda: path.write_bytes(b"new fixture\n"))
        elif operation == "chown":
            results["cap-chown-operation"] = attempt(lambda: os.chown(path, 1234, 2345))
        elif operation == "chmod":
            results["cap-chmod-operation"] = attempt(lambda: os.chmod(path, 0o640))
        results["cap-" + operation + "-after"] = attempt(
            lambda: os.getxattr(path, "security.capability").hex())

# Only synthetic files are mutated. Never open the device node.
results["device-node"] = attempt(
    lambda: os.mknod(base / "device", stat.S_IFCHR | 0o600, os.makedev(1, 3)))
for name, bit in (("nodump", 0x40), ("immutable", 0x10), ("append", 0x20)):
    path = base / ("flag-" + name)
    path.write_bytes(b"flag fixture\n")
    before = flags(path)
    results[name + "-set"] = attempt(lambda: flags(path, before | bit))
    results[name + "-get"] = attempt(lambda: flags(path))
    if results[name + "-set"] == {"ok": None}:
        try:
            def write(append):
                fd = os.open(path, os.O_WRONLY | (os.O_APPEND if append else 0))
                try:
                    os.write(fd, b"bounded fixture write\n")
                finally:
                    os.close(fd)
            results[name + "-write"] = attempt(lambda: write(False))
            results[name + "-append"] = attempt(lambda: write(True))
            results[name + "-chmod"] = attempt(lambda: os.chmod(path, 0o600))
        finally:
            # Leave nodump set to check persistence, clear restrictive bits.
            if name != "nodump":
                flags(path, before)
# Bounded allocation/extent operations on disposable regular files only.
# Linux fallocate modes from linux/falloc.h, x86_64 off_t is signed 64-bit.
libc = ctypes.CDLL(None, use_errno=True)
libc.fallocate.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_int64, ctypes.c_int64]
libc.fallocate.restype = ctypes.c_int
payload = b"A" * 16384 + b"B" * 16384 + b"C" * 16384 + b"D" * 16384
for name, mode, offset, length, expected in (
    ("allocate", 0, 65536, 65536, payload + bytes(65536)),
    ("punch", 3, 16384, 16384, payload[:16384] + bytes(16384) + payload[32768:]),
    ("zero", 16, 16384, 16384, payload[:16384] + bytes(16384) + payload[32768:]),
    ("collapse", 8, 16384, 16384, payload[:16384] + payload[32768:]),
):
    path = base / ("io-" + name)
    path.write_bytes(payload)
    with path.open("r+b") as stream:
        os.fsync(stream.fileno())
        blocks_before = path.stat().st_blocks
        def allocate():
            if libc.fallocate(stream.fileno(), mode, offset, length):
                code = ctypes.get_errno()
                raise OSError(code, os.strerror(code))
        outcome = attempt(allocate)
        results["io-" + name] = outcome
        os.fsync(stream.fileno())
        results["io-" + name + "-blocks-delta"] = path.stat().st_blocks - blocks_before
        assert path.read_bytes() == (expected if outcome == {"ok": None} else payload), name
        results["io-" + name + "-seek-data"] = attempt(lambda: os.lseek(stream.fileno(), 16384, os.SEEK_DATA))
        results["io-" + name + "-seek-hole"] = attempt(lambda: os.lseek(stream.fileno(), 16384, os.SEEK_HOLE))
# Unaligned ranges must preserve both edge blocks outside the requested bytes.
for name, mode, offset, length in (
    ("punch-middle", 3, 123, 9001),
    ("zero-middle", 16, 123, 9001),
    ("punch-single-block", 3, 123, 19),
    ("zero-single-block", 16, 123, 19),
    ("punch-past-eof", 3, 65500, 9001),
    ("zero-past-eof", 16, 65500, 9001),
):
    path = base / ("io-unaligned-" + name)
    path.write_bytes(payload)
    with path.open("r+b") as stream:
        def unaligned():
            if libc.fallocate(stream.fileno(), mode, offset, length):
                code = ctypes.get_errno()
                raise OSError(code, os.strerror(code))
        results["range-" + name] = attempt(unaligned)
        assert results["range-" + name] == {"ok": None}, (name, results["range-" + name])
        end = min(offset + length, len(payload)) if mode == 3 else offset + length
        expected = payload[:offset] + bytes(end - offset) + payload[end:]
        os.fsync(stream.fileno())
        assert path.read_bytes() == expected, name
print("UNALIGNED_RANGE_CONTENT_CHECKS_PASS", flush=True)
source, destination = base / "io-clone-source", base / "io-clone-destination"
source.write_bytes(payload)
destination.write_bytes(b"")
with source.open("rb") as src, destination.open("r+b") as dst:
    # FICLONE from linux/fs.h, ioctl argument is an existing regular-file FD.
    results["io-reflink"] = attempt(lambda: fcntl.ioctl(dst.fileno(), 0x40049409, src.fileno()))
    os.fsync(dst.fileno())
    assert destination.read_bytes() == (payload if "ok" in results["io-reflink"] else b"")
assert source.read_bytes() == payload
print("BOUNDED_EXTENT_CONTENT_CHECKS_PASS", flush=True)

report.write_text(json.dumps({"operations": results, "disk": snapshot()},
                            sort_keys=True) + "\n")
print("METADATA_OBSERVATIONS " + json.dumps(results, sort_keys=True), flush=True)
