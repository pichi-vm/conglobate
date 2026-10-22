#!/usr/bin/env python3
"""Boot Debian UML with a benign initramfs; measure host-observed PID1 readiness."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import stat
import subprocess
import tarfile
import tempfile
import time

INIT = b'''#!/bin/sh
set -eu
test "$$" = 1
/bin/busybox mount -t proc proc /proc
echo UML_BOOT_READY
/bin/busybox uname -a
/bin/busybox cat /proc/uptime
/bin/busybox mount -t sysfs sysfs /sys
/bin/busybox mount -t securityfs securityfs /sys/kernel/security
test "$(/bin/busybox cat /sys/kernel/security/lsm)" = capability
echo UML_LSM_CAPABILITY_ONLY
/bin/busybox cat /proc/1/status
echo UML_BOOT_COMPLETE
/bin/busybox poweroff -f
'''


def archive(busybox):
    result = bytearray()
    entries = [("bin", stat.S_IFDIR | 0o755, b"", 0, 0),
               ("dev", stat.S_IFDIR | 0o755, b"", 0, 0),
               ("proc", stat.S_IFDIR | 0o755, b"", 0, 0),
               ("sys", stat.S_IFDIR | 0o755, b"", 0, 0),
               ("dev/console", stat.S_IFCHR | 0o600, b"", 5, 1),
               ("dev/null", stat.S_IFCHR | 0o666, b"", 1, 3),
               ("bin/busybox", stat.S_IFREG | 0o755, busybox, 0, 0),
               ("bin/sh", stat.S_IFLNK | 0o777, b"busybox", 0, 0),
               ("init", stat.S_IFREG | 0o755, INIT, 0, 0),
               ("TRAILER!!!", 0, b"", 0, 0)]
    for inode, (name, mode, data, major, minor) in enumerate(entries, 1):
        name = name.encode() + b"\0"
        fields = [inode, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(name), 0]
        result += b"070701" + b"".join(f"{x:08x}".encode() for x in fields) + name
        result += b"\0" * (-len(result) % 4)
        result += data
        result += b"\0" * (-len(result) % 4)
    return result


def extract(deb, member, destination):
    process = subprocess.Popen(["dpkg-deb", "--fsys-tarfile", str(deb)], stdout=subprocess.PIPE)
    found = False
    with tarfile.open(fileobj=process.stdout, mode="r|") as tar:
        for item in tar:
            if item.name == member:
                with destination.open("wb") as output:
                    shutil.copyfileobj(tar.extractfile(item), output)
                found = True
    process.stdout.close()
    if process.wait() != 0 or not found:
        raise RuntimeError(f"could not extract {member}")
    destination.chmod(0o755)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--packages", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5)
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(exist_ok=False)
    rows = json.loads((args.packages / "packages.json").read_text())
    for name, member, dest in [("user-mode-linux", "./usr/bin/linux.uml", "linux.uml"),
                                ("busybox-static", "./usr/bin/busybox", "busybox")]:
        row = next(r for r in rows if r["Package"] == name)
        deb = args.packages / Path(row["Filename"]).name
        assert hashlib.sha256(deb.read_bytes()).hexdigest() == row["SHA256"]
        extract(deb, member, args.out / dest)
    (args.out / "init").write_bytes(INIT)
    (args.out / "initramfs.cpio").write_bytes(archive((args.out / "busybox").read_bytes()))
    metadata = {"packages": rows, "host": list(os.uname()), "uid": os.getuid(),
                "clock": "time.monotonic_ns", "memory": "128M", "runs": [],
                "method": "parent immediately before Popen to receipt of exact PID1 marker line; no cache flushing"}
    for name in ["linux.uml", "busybox", "initramfs.cpio"]:
        metadata[name] = hashlib.sha256((args.out / name).read_bytes()).hexdigest()
    for number in range(args.runs):
        runtime = Path(tempfile.mkdtemp(prefix="conglobate-uml-boot-", dir="/dev/shm"))
        command = [str(args.out / "linux.uml"), "mem=128M", "con=null", "con0=fd:0,fd:1",
                   "ssl=null", "lsm=capability", "panic=1", "rdinit=/init",
                   f"initrd={args.out / 'initramfs.cpio'}", f"uml_dir={runtime}", f"umid=run{number}"]
        env = {"PATH": "/usr/bin:/bin", "HOME": str(runtime), "TMPDIR": str(runtime), "LC_ALL": "C"}
        started = time.monotonic_ns()
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, env=env, start_new_session=True)
        output, pending = bytearray(), bytearray()
        ready, complete, error = None, False, None
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(proc.stdout, selectors.EVENT_READ)
                while selector.get_map():
                    if time.monotonic_ns() - started > 30_000_000_000:
                        raise TimeoutError("30-second boot deadline")
                    for key, _ in selector.select(0.1):
                        chunk = os.read(key.fd, 65536)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            break
                        output += chunk
                        pending += chunk
                        if len(output) > 2 * 1024 * 1024:
                            raise RuntimeError("unexpected output volume")
                        while b"\n" in pending:
                            line, _, remainder = pending.partition(b"\n")
                            pending = bytearray(remainder)
                            if line.strip() == b"UML_BOOT_READY":
                                if ready is not None:
                                    raise RuntimeError("duplicate readiness marker")
                                ready = (time.monotonic_ns() - started) / 1e9
                            if line.strip() == b"UML_BOOT_COMPLETE":
                                complete = True
            proc.wait(timeout=2)
        except Exception as exc:
            error = repr(exc)
        finally:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.wait(timeout=5)
            proc.stdin.close()
            proc.stdout.close()
        elapsed = (time.monotonic_ns() - started) / 1e9
        (args.out / f"run-{number}.log").write_bytes(output)
        record = {"number": number, "command": command, "ready_seconds": ready,
                  "total_seconds": elapsed, "returncode": proc.returncode,
                  "complete": complete, "error": error}
        metadata["runs"].append(record)
        (args.out / "results.json").write_text(json.dumps(metadata, indent=2) + "\n")
        shutil.rmtree(runtime)
        print(json.dumps(record), flush=True)
        if error or ready is None or not complete or proc.returncode != 0:
            raise SystemExit("Boot did not meet success criteria; see retained log")


if __name__ == "__main__":
    main()
